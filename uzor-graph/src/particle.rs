//! Simulated point-mass state — kept in a separate SoA-style array from
//! node payload so the physics loop never touches app data every tick.
//! Mirrors d3-force's own `{x, y, vx, vy, fx, fy}` node fields (see the
//! engine design doc §2.3/§3.1).

/// One simulated point mass.
///
/// `fx`/`fy` hold the pinned position (`Some` while a node is being
/// dragged, or persistently after an explicit pin). When set, the
/// integrator skips force accumulation for that axis and holds the
/// value directly — the same `fx`/`fy` convention d3-force uses. This is
/// the single source of truth for "is this node pinned" — no separate
/// `Layout::pin`/`unpin` bookkeeping is needed since any code that can
/// see the particle slice can set/clear these fields directly.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub fx: Option<f32>,
    pub fy: Option<f32>,
}

impl Particle {
    /// Construct a particle at rest at `(x, y)`.
    pub fn at(x: f32, y: f32) -> Self {
        Self { x, y, ..Default::default() }
    }

    pub fn is_pinned(&self) -> bool {
        self.fx.is_some() || self.fy.is_some()
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

    /// Release the pin — the particle rejoins the simulation next tick.
    pub fn unpin(&mut self) {
        self.fx = None;
        self.fy = None;
    }
}
