//! `Camera3D` — shared orbit and free-look state for 3D graph mode,
//! producing a fresh `uzor_urx_3d::PerspectiveCamera` each frame.
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

/// Camera state shared by orbit and fly-style controls. In orbit use,
/// `target` is the fixed center and `distance` is the radius. In free-look
/// use, `eye()` is the fixed camera position and `target` is an aim point
/// on its forward axis at `distance`.
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

    /// Unit vector from the camera position through the center of the
    /// viewport. This is the axis a captured-mouse crosshair represents.
    pub fn forward(&self) -> Vec3 {
        -self.orbit_offset().normalize_or_zero()
    }

    /// Camera-local right axis with world-Y kept as the stable up reference.
    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize_or_zero()
    }

    /// Fresh `PerspectiveCamera` for the current orbit state — `up` stays
    /// world-`Y` (`PerspectiveCamera::new`'s own default), fov also
    /// default; `eye`/`target`/`aspect` vary per frame, and so do
    /// `z_near`/`z_far` (see the divergence note below).
    ///
    /// **Wave 2 divergence (`uzor-graph/CLAUDE.md`)**: `PerspectiveCamera::new`
    /// hardcodes `z_near = 0.1` / `z_far = 100.0` — tuned for
    /// `uzor-urx-3d`'s own small-scene demos (every example/test camera
    /// sits 3-7 world units from the origin). Graph world-space spans
    /// hundreds of units at this crate's default `distance = 500.0`
    /// (`[MIN_DISTANCE, MAX_DISTANCE]` = `[1.0, 100_000.0]`) — a fixed
    /// `z_far = 100.0` would clip the orbit TARGET itself the moment
    /// `distance` exceeds ~100, well inside this camera's normal range.
    /// `PerspectiveCamera`'s fields are `pub` (`uzor-urx-3d/src/camera.rs`),
    /// so both planes are overridden here, scaled to `distance`, with no
    /// `uzor-urx-3d` change: `z_far` comfortably contains the target plus
    /// a margin for nodes spread around it, `z_near` shrinks with `distance`
    /// so dollying in close never clips the target either.
    pub fn to_perspective(&self, aspect: f32) -> PerspectiveCamera {
        let mut camera = PerspectiveCamera::new(self.eye(), self.target, aspect);
        camera.z_near = (self.distance * 0.001).max(0.05);
        camera.z_far = (self.distance * 4.0).max(2_000.0);
        camera
    }

    /// Drag-to-orbit: `delta_x`/`delta_y` are raw screen-pixel deltas,
    /// converted internally via [`ORBIT_RADIANS_PER_PX`] (plan §1.4).
    /// Pitch is clamped to [`PITCH_LIMIT`] so the camera can never flip
    /// past looking straight up/down.
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * ORBIT_RADIANS_PER_PX;
        self.pitch = (self.pitch + delta_y * ORBIT_RADIANS_PER_PX).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Captured-mouse free look. Unlike [`Self::orbit`], this preserves the
    /// camera's world-space eye position and rotates the forward axis through
    /// it. `target` becomes the moving aim point at the existing `distance`.
    pub fn free_look(&mut self, delta_x: f32, delta_y: f32) {
        let eye = self.eye();
        self.orbit(delta_x, delta_y);
        self.target = eye - self.orbit_offset();
    }

    /// Translate the complete camera frame in local units. Eye and aim point
    /// move by the same vector, so orientation and focus distance are stable.
    pub fn translate_local(&mut self, right: f32, up: f32, forward: f32) {
        let forward_axis = self.forward();
        let right_axis = self.right();
        let up_axis = right_axis.cross(forward_axis).normalize_or_zero();
        self.target += right_axis * right + up_axis * up + forward_axis * forward;
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
        let forward = self.forward();
        let right = self.right();
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
    fn free_look_rotates_forward_axis_without_orbiting_the_eye() {
        let mut camera = Camera3D {
            target: Vec3::new(12.0, -4.0, 8.0),
            distance: 25.0,
            yaw: 0.3,
            pitch: -0.2,
        };
        let eye_before = camera.eye();
        let target_before = camera.target;
        let forward_before = camera.forward();

        camera.free_look(40.0, -15.0);

        assert!((camera.eye() - eye_before).length() < 1e-4, "free look must preserve camera position");
        assert!((camera.target - target_before).length() > 1.0, "the aim point must move with orientation");
        assert!((camera.forward() - forward_before).length() > 0.1, "the forward axis must rotate");
        assert!(((camera.target - camera.eye()).length() - camera.distance).abs() < 1e-4);
    }

    #[test]
    fn local_translation_moves_eye_and_aim_together() {
        let mut camera = Camera3D {
            target: Vec3::new(-3.0, 5.0, 9.0),
            distance: 18.0,
            yaw: 0.7,
            pitch: 0.25,
        };
        let eye_before = camera.eye();
        let target_before = camera.target;
        let forward_before = camera.forward();

        camera.translate_local(6.0, -2.0, 11.0);

        let eye_delta = camera.eye() - eye_before;
        let target_delta = camera.target - target_before;
        assert!((eye_delta - target_delta).length() < 1e-4);
        assert!((camera.forward() - forward_before).length() < 1e-6);
        assert!(((camera.target - camera.eye()).length() - camera.distance).abs() < 1e-4);
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
