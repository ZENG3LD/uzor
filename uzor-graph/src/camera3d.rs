//! `Camera3D` — orbit-camera state for 3D graph mode, producing a fresh
//! `uzor_urx_3d::PerspectiveCamera` each frame. Pure math this wave (W3D
//! arc plan §1.4/§4 Wave 1) — event wiring (drag/wheel/pan input
//! dispatch) lands in Wave 2.
//!
//! `Camera2D` is the house precedent for this split (plan §1.4's own
//! reasoning): interaction state lives in `uzor-graph`, the render
//! primitive (`PerspectiveCamera`) stays a generic-renderer type with no
//! graph-specific interaction semantics baked in — this is deliberately
//! NOT a method added onto `PerspectiveCamera` itself.

use glam::Vec3;
use uzor_urx_3d::PerspectiveCamera;

/// Screen-px -> radians conversion for [`Camera3D::orbit`] — mirrors the
/// 2D engine's own "delta ÷ zoom" drag-tick convention (W2.1), just with
/// a fixed divisor instead of a variable zoom (orbit has no zoom-
/// equivalent axis of its own).
const ORBIT_RADIANS_PER_PX: f32 = 0.008;

/// Pitch clamp — stays strictly inside ±90° so [`Camera3D::orbit_offset`]'s
/// `right = forward.cross(Vec3::Y)` never degenerates (a forward vector
/// exactly parallel to the world-up axis has no well-defined right).
const PITCH_LIMIT: f32 = 1.483_53; // ~85 degrees, radians

const MIN_DISTANCE: f32 = 1.0;
const MAX_DISTANCE: f32 = 100_000.0;

/// Pan speed scales with `distance` (further from `target` => a screen
/// pixel spans more world space) — the standard orbit-camera convention
/// (three.js `OrbitControls`, Blender's own viewport nav) so a shift-drag
/// pan feels the same speed regardless of current zoom/distance.
const PAN_UNITS_PER_PX_PER_DISTANCE: f32 = 0.002;

/// Orbit-camera state: `target` is the look-at point, `distance` the
/// orbit radius, `yaw`/`pitch` the spherical angles (radians) around it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera3D {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self { target: Vec3::ZERO, distance: 500.0, yaw: 0.0, pitch: 0.35 }
    }
}

impl Camera3D {
    /// `eye = target + distance * spherical(yaw, pitch)` (plan §1.4) —
    /// yaw rotates around the world-up (`Y`) axis, pitch tilts up/down
    /// from the horizontal plane.
    fn orbit_offset(&self) -> Vec3 {
        let (sp, cp) = self.pitch.sin_cos();
        let (sy, cy) = self.yaw.sin_cos();
        Vec3::new(self.distance * cp * sy, self.distance * sp, self.distance * cp * cy)
    }

    /// World-space eye position for the current orbit state.
    pub fn eye(&self) -> Vec3 {
        self.target + self.orbit_offset()
    }

    /// Fresh `PerspectiveCamera` for the current orbit state — `up` stays
    /// world-`Y` (`PerspectiveCamera::new`'s own default), fov/near/far
    /// also default; only `eye`/`target`/`aspect` vary per frame.
    pub fn to_perspective(&self, aspect: f32) -> PerspectiveCamera {
        PerspectiveCamera::new(self.eye(), self.target, aspect)
    }

    /// Drag-to-orbit: `delta_x`/`delta_y` are raw screen-pixel deltas,
    /// converted internally via [`ORBIT_RADIANS_PER_PX`] (plan §1.4).
    /// Pitch is clamped to [`PITCH_LIMIT`] so the camera can never flip
    /// past looking straight up/down.
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * ORBIT_RADIANS_PER_PX;
        self.pitch = (self.pitch + delta_y * ORBIT_RADIANS_PER_PX).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Wheel-to-dolly: multiplicative distance change, clamped to
    /// [`MIN_DISTANCE`, `MAX_DISTANCE`] (plan §1.4).
    pub fn dolly(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Shift/middle-drag-to-pan: moves `target` in the camera's own
    /// right/up plane (plan §1.4), scaled by `distance` so pan speed
    /// feels zoom-independent (see [`PAN_UNITS_PER_PX_PER_DISTANCE`]).
    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let forward = -self.orbit_offset().normalize_or_zero();
        let right = forward.cross(Vec3::Y).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let scale = PAN_UNITS_PER_PX_PER_DISTANCE * self.distance;
        self.target -= right * (delta_x * scale);
        self.target += up * (delta_y * scale);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_perspective_places_the_eye_at_distance_from_target_along_the_orbit_offset() {
        let camera = Camera3D { target: Vec3::ZERO, distance: 10.0, yaw: 0.0, pitch: 0.0 };
        let persp = camera.to_perspective(16.0 / 9.0);
        assert!((persp.eye - Vec3::new(0.0, 0.0, 10.0)).length() < 1e-4);
        assert_eq!(persp.target, Vec3::ZERO);
    }

    #[test]
    fn orbit_advances_yaw_and_clamps_pitch_within_limits() {
        let mut camera = Camera3D { yaw: 0.0, pitch: 0.0, ..Camera3D::default() };
        camera.orbit(100.0, 100_000.0);
        assert!(camera.yaw > 0.0);
        assert!(camera.pitch <= PITCH_LIMIT + 1e-6);

        camera.pitch = 0.0;
        camera.orbit(0.0, -100_000.0);
        assert!(camera.pitch >= -PITCH_LIMIT - 1e-6);
    }

    #[test]
    fn dolly_scales_distance_and_clamps_within_bounds() {
        let mut camera = Camera3D { distance: 10.0, ..Camera3D::default() };
        camera.dolly(2.0);
        assert!((camera.distance - 20.0).abs() < 1e-4);

        camera.dolly(1e12);
        assert!(camera.distance <= MAX_DISTANCE);

        camera.distance = 10.0;
        camera.dolly(1e-12);
        assert!(camera.distance >= MIN_DISTANCE);
    }

    #[test]
    fn pan_moves_the_target_and_leaves_distance_untouched() {
        let mut camera = Camera3D { target: Vec3::ZERO, distance: 10.0, yaw: 0.0, pitch: 0.0 };
        camera.pan(50.0, 0.0);
        assert!(camera.target.length() > 0.0);
        assert!((camera.distance - 10.0).abs() < 1e-6);
    }
}
