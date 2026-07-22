//! `FlyController` — generic WASD/arrow-key + captured-mouse fly
//! navigation for any [`crate::camera3d::Camera3D`] consumer.
//!
//! Lifted from `foxhound-app-shell-native`'s own game-style navigation
//! loop (`nemo/foxhound/crates/foxhound-app-shell-native/src/main.rs`'s
//! `MovementKeys` + `foxhound-app/src/lib.rs`'s `tick_navigation`/
//! `set_navigation_input`/`free_look_view` — see
//! `nemo/docs/uzor-engines/research_foxhound_lift_candidates.md` §2)
//! per the LIFT-WITH-REWORK verdict there: the key-state tracker, the
//! exponential ease-in/ease-out inertia model, and the keyboard/mouse
//! sensitivity scalars are 100% generic — zero Foxhound-specific types —
//! and are reproduced here unchanged in their math. What did NOT come
//! along is Foxhound's `NavigationFrame::Axes` semantic (an axis-locked
//! time/entity-lane pan interpretation that is domain-specific to that
//! app's own forensic projection): this controller's only built-in pan
//! style is the source app's `NavigationFrame::Viewport` behavior —
//! [`Camera3D::translate_local`] — which was already 100% generic. A
//! caller that wants a different semantic mapping (e.g. an axis-locked
//! pan) should read [`FlyController::velocity`] after calling
//! [`FlyController::tick`] and apply its own dispatch instead of calling
//! `tick`'s camera-mutating form.
//!
//! Generic API: feed [`uzor::input::PlatformEvent`] key/pointer-delta
//! events via [`FlyController::on_event`] (or the finer-grained
//! [`FlyController::set_key`]/[`FlyController::apply_look_delta`]), then
//! call [`FlyController::tick`] once per frame to advance inertia and
//! move a `&mut Camera3D`.

use uzor::input::{KeyCode, PlatformEvent};

use crate::camera3d::Camera3D;

/// Legacy-arrow-step convention, matching the source app's own comment:
/// velocity is expressed as "steps per second" so the resulting distance
/// math (which multiplies by `camera.distance`, not a fixed world unit)
/// keeps working across wildly different scene scales.
const NAVIGATION_BASE_STEPS_PER_SECOND: f32 = 4.0;
const NAVIGATION_ACCEL_RESPONSE: f32 = 18.0;
const NAVIGATION_DECEL_RESPONSE: f32 = 22.0;
const NAVIGATION_STOP_EPSILON: f32 = 0.005;
/// `tick`'s own `dt` clamp — guards against a huge inertia jump after a
/// stall (e.g. a debugger breakpoint or a dropped frame), same value the
/// source app used.
const MAX_TICK_DT: f32 = 0.05;

/// Strafe (local-right) distance scale, applied to `velocity[0] * dt *
/// camera.distance` — the source app's own `VIEWPORT_STRAFE_DISTANCE`.
const STRAFE_DISTANCE_SCALE: f32 = 0.12;
/// Forward/back (local-forward) distance scale — the source app's own
/// `VIEWPORT_ARROW_FORWARD_DISTANCE`.
const FORWARD_DISTANCE_SCALE: f32 = 0.08;

pub const KEYBOARD_SENSITIVITY_MIN: f32 = 0.25;
pub const KEYBOARD_SENSITIVITY_MAX: f32 = 2.5;
pub const MOUSE_SENSITIVITY_MIN: f32 = 0.2;
pub const MOUSE_SENSITIVITY_MAX: f32 = 2.5;

/// Held-key state for WASD/arrow-key navigation — generic left/right +
/// forward/backward axis tracking, lifted verbatim from the source app's
/// own `MovementKeys`.
#[derive(Debug, Default, Clone, Copy)]
struct MovementKeys {
    left: bool,
    right: bool,
    forward: bool,
    backward: bool,
}

impl MovementKeys {
    /// Update held state for a recognized WASD/arrow key. Returns `false`
    /// (and leaves state untouched) for any other key.
    fn set(&mut self, key: KeyCode, pressed: bool) -> bool {
        let target = match key {
            KeyCode::ArrowLeft | KeyCode::A => &mut self.left,
            KeyCode::ArrowRight | KeyCode::D => &mut self.right,
            KeyCode::ArrowUp | KeyCode::W => &mut self.forward,
            KeyCode::ArrowDown | KeyCode::S => &mut self.backward,
            _ => return false,
        };
        *target = pressed;
        true
    }

    /// `(horizontal, vertical)` in `[-1, 1]` each — opposite keys held
    /// together cancel to `0.0`.
    fn vector(&self) -> (f32, f32) {
        let horizontal = f32::from(u8::from(self.right)) - f32::from(u8::from(self.left));
        let vertical = f32::from(u8::from(self.forward)) - f32::from(u8::from(self.backward));
        (horizontal, vertical)
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Game-style WASD/arrow-key + captured-mouse fly navigation over a
/// [`Camera3D`] — see the module doc for exactly what was lifted and
/// what stayed app-side.
#[derive(Debug, Clone, Copy)]
pub struct FlyController {
    movement_keys: MovementKeys,
    /// Clamped 2D held-intent vector (`[horizontal, vertical]`), each in
    /// `[-1, 1]`, diagonal-normalized to one input unit.
    input: [f32; 2],
    /// Inertial "legacy arrow steps per second" velocity — see
    /// [`NAVIGATION_BASE_STEPS_PER_SECOND`]'s own doc comment.
    velocity: [f32; 2],
    keyboard_sensitivity: f32,
    mouse_sensitivity: f32,
}

impl Default for FlyController {
    fn default() -> Self {
        Self {
            movement_keys: MovementKeys::default(),
            input: [0.0; 2],
            velocity: [0.0; 2],
            keyboard_sensitivity: 1.0,
            mouse_sensitivity: 1.0,
        }
    }
}

impl FlyController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current held 2D intent (`[horizontal, vertical]`), post diagonal
    /// normalization — mostly useful for tests/telemetry.
    pub fn input(&self) -> [f32; 2] {
        self.input
    }

    /// Current inertial velocity (`[horizontal, vertical]`, legacy
    /// "arrow steps per second" units) — exposed so a caller can publish
    /// it (e.g. over agent-api) or build its own pan dispatch on top of
    /// [`FlyController::tick`]'s per-tick step instead of the built-in
    /// `translate_local` mapping.
    pub fn velocity(&self) -> [f32; 2] {
        self.velocity
    }

    pub fn keyboard_sensitivity(&self) -> f32 {
        self.keyboard_sensitivity
    }

    pub fn set_keyboard_sensitivity(&mut self, sensitivity: f32) {
        self.keyboard_sensitivity = sensitivity.clamp(KEYBOARD_SENSITIVITY_MIN, KEYBOARD_SENSITIVITY_MAX);
    }

    pub fn mouse_sensitivity(&self) -> f32 {
        self.mouse_sensitivity
    }

    pub fn set_mouse_sensitivity(&mut self, sensitivity: f32) {
        self.mouse_sensitivity = sensitivity.clamp(MOUSE_SENSITIVITY_MIN, MOUSE_SENSITIVITY_MAX);
    }

    /// Track a held WASD/arrow key and fold it into the current 2D
    /// intent — fuses the source app's own `MovementKeys::update` +
    /// `set_navigation_input` into one call. Returns `true` if `key` was
    /// part of the navigation key set (a caller's `on_event` can
    /// early-return on `true`, exactly like the source app did).
    pub fn set_key(&mut self, key: KeyCode, pressed: bool) -> bool {
        if !self.movement_keys.set(key, pressed) {
            return false;
        }
        let (horizontal, vertical) = self.movement_keys.vector();
        self.set_input(horizontal, vertical);
        true
    }

    /// Directly set held intent, bypassing key tracking — e.g. for a
    /// caller driving navigation from a virtual joystick/gamepad stick.
    /// Diagonals are normalized to one input unit (the source app's own
    /// `set_navigation_input`).
    pub fn set_input(&mut self, horizontal: f32, vertical: f32) {
        let mut input = [horizontal.clamp(-1.0, 1.0), vertical.clamp(-1.0, 1.0)];
        let length = (input[0] * input[0] + input[1] * input[1]).sqrt();
        if length > 1.0 {
            input[0] /= length;
            input[1] /= length;
        }
        self.input = input;
    }

    /// Immediately clear held keys, intent, and inertia — call this on
    /// focus loss (or anywhere a `KeyUp` might be missed) so the camera
    /// can never keep drifting. Mirrors the source app's own
    /// `stop_movement`/`stop_navigation` pair.
    pub fn stop(&mut self) {
        self.movement_keys.clear();
        self.input = [0.0; 2];
        self.velocity = [0.0; 2];
    }

    /// Apply captured-mouse motion as free look, scaled by
    /// [`FlyController::mouse_sensitivity`] — the source app's own
    /// `free_look_view`.
    pub fn apply_look_delta(&self, delta_x: f32, delta_y: f32, camera: &mut Camera3D) {
        camera.free_look(-delta_x * self.mouse_sensitivity, delta_y * self.mouse_sensitivity);
    }

    /// Advance inertial keyboard navigation with exponential ease-in/
    /// ease-out (the source app's own `tick_navigation`), then dispatch
    /// the resulting per-axis step into [`Camera3D::translate_local`] —
    /// strafe on the local-right axis, forward/back on the local-forward
    /// axis. This IS the controller's one built-in pan style (the source
    /// app's `NavigationFrame::Viewport`) — see the module doc for why a
    /// different semantic mapping stays entirely the caller's concern.
    pub fn tick(&mut self, dt: f32, camera: &mut Camera3D) {
        let dt = dt.clamp(0.0, MAX_TICK_DT);
        for axis in 0..2 {
            let target = self.input[axis] * NAVIGATION_BASE_STEPS_PER_SECOND * self.keyboard_sensitivity;
            let response = if self.input[axis].abs() > f32::EPSILON {
                NAVIGATION_ACCEL_RESPONSE
            } else {
                NAVIGATION_DECEL_RESPONSE
            };
            let blend = 1.0 - (-response * dt).exp();
            self.velocity[axis] += (target - self.velocity[axis]) * blend;
            if target == 0.0 && self.velocity[axis].abs() < NAVIGATION_STOP_EPSILON {
                self.velocity[axis] = 0.0;
            }
        }

        let strafe_step = self.velocity[0] * dt;
        let forward_step = self.velocity[1] * dt;
        if strafe_step != 0.0 || forward_step != 0.0 {
            let strafe_distance = strafe_step * camera.distance * STRAFE_DISTANCE_SCALE;
            let forward_distance = forward_step * camera.distance * FORWARD_DISTANCE_SCALE;
            camera.translate_local(strafe_distance, 0.0, forward_distance);
        }
    }

    /// Ergonomic single ingestion point for the two event kinds this
    /// controller cares about: `KeyDown`/`KeyUp` always update held
    /// intent (returns `true` only for a recognized WASD/arrow key);
    /// `PointerDelta` applies free look ONLY while `mouse_look_active`
    /// (the caller's own cursor-capture gate — this controller has no
    /// opinion on `CursorCaptureMode`). Every other event is ignored
    /// (`false`). A caller that wants finer control (e.g. driving intent
    /// from a source other than `PlatformEvent`) should call
    /// [`FlyController::set_key`]/[`FlyController::apply_look_delta`]
    /// directly instead.
    pub fn on_event(&mut self, event: &PlatformEvent, mouse_look_active: bool, camera: &mut Camera3D) -> bool {
        match event {
            PlatformEvent::KeyDown { key, .. } => self.set_key(*key, true),
            PlatformEvent::KeyUp { key, .. } => self.set_key(*key, false),
            PlatformEvent::PointerDelta { dx, dy } if mouse_look_active => {
                self.apply_look_delta(*dx as f32, *dy as f32, camera);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::input::ModifierKeys;

    fn key_down(key: KeyCode) -> PlatformEvent {
        PlatformEvent::KeyDown { key, modifiers: ModifierKeys::default() }
    }

    fn key_up(key: KeyCode) -> PlatformEvent {
        PlatformEvent::KeyUp { key, modifiers: ModifierKeys::default() }
    }

    fn converge(controller: &mut FlyController, camera: &mut Camera3D, seconds: f32) {
        let dt = 1.0 / 60.0;
        let mut elapsed = 0.0;
        while elapsed < seconds {
            controller.tick(dt, camera);
            elapsed += dt;
        }
    }

    #[test]
    fn set_key_recognizes_wasd_and_arrow_variants_and_rejects_other_keys() {
        let mut controller = FlyController::new();
        assert!(controller.set_key(KeyCode::W, true));
        assert_eq!(controller.input(), [0.0, 1.0]);
        assert!(controller.set_key(KeyCode::ArrowRight, true));
        assert!((controller.input()[0] - 1.0 / 2.0f32.sqrt()).abs() < 1e-6);
        assert!(!controller.set_key(KeyCode::Escape, true), "a non-navigation key must not be consumed");
    }

    #[test]
    fn opposite_keys_cancel_to_zero_input_and_settle_to_zero_velocity() {
        let mut controller = FlyController::new();
        let mut camera = Camera3D::default();
        controller.set_key(KeyCode::W, true);
        controller.set_key(KeyCode::S, true);
        assert_eq!(controller.input(), [0.0, 0.0], "forward+backward held together must cancel");

        converge(&mut controller, &mut camera, 1.0);
        assert!(controller.velocity()[1].abs() < 1e-4, "velocity must settle to zero with cancelled input");
        assert!(camera.target.length() < 1e-3, "camera must not drift with cancelled input");
    }

    #[test]
    fn pressing_forward_then_ticking_moves_the_camera_along_its_own_forward_axis() {
        let mut controller = FlyController::new();
        let mut camera = Camera3D::default();
        let forward_axis = camera.forward();

        controller.set_key(KeyCode::W, true);
        converge(&mut controller, &mut camera, 1.0);

        assert!(controller.velocity()[1] > 0.0, "forward velocity must build up under held W");
        assert!(
            camera.target.dot(forward_axis) > 0.0,
            "the camera must have moved along its own (orientation-invariant) forward axis"
        );
    }

    #[test]
    fn releasing_a_key_decays_velocity_via_inertia_instead_of_snapping_to_zero() {
        let mut controller = FlyController::new();
        let mut camera = Camera3D::default();
        controller.set_key(KeyCode::W, true);
        converge(&mut controller, &mut camera, 1.0);
        let steady_state = controller.velocity()[1];
        assert!(steady_state > 0.0);

        controller.set_key(KeyCode::W, false);
        controller.tick(1.0 / 60.0, &mut camera);
        let after_one_tick = controller.velocity()[1];

        assert!(after_one_tick > 0.0, "velocity must not snap to zero the instant the key releases");
        assert!(after_one_tick < steady_state, "velocity must be decaying toward zero after release");
    }

    #[test]
    fn keyboard_sensitivity_scales_the_converged_steady_state_velocity() {
        let mut default_controller = FlyController::new();
        let mut default_camera = Camera3D::default();
        default_controller.set_key(KeyCode::W, true);
        converge(&mut default_controller, &mut default_camera, 2.0);

        let mut fast_controller = FlyController::new();
        fast_controller.set_keyboard_sensitivity(2.0);
        let mut fast_camera = Camera3D::default();
        fast_controller.set_key(KeyCode::W, true);
        converge(&mut fast_controller, &mut fast_camera, 2.0);

        let ratio = fast_controller.velocity()[1] / default_controller.velocity()[1];
        assert!((ratio - 2.0).abs() < 1e-2, "2x keyboard sensitivity must converge to ~2x the steady-state velocity, got ratio {ratio}");
    }

    #[test]
    fn set_keyboard_sensitivity_clamps_to_the_documented_range() {
        let mut controller = FlyController::new();
        controller.set_keyboard_sensitivity(100.0);
        assert_eq!(controller.keyboard_sensitivity(), KEYBOARD_SENSITIVITY_MAX);
        controller.set_keyboard_sensitivity(-5.0);
        assert_eq!(controller.keyboard_sensitivity(), KEYBOARD_SENSITIVITY_MIN);
    }

    #[test]
    fn set_mouse_sensitivity_clamps_to_the_documented_range() {
        let mut controller = FlyController::new();
        controller.set_mouse_sensitivity(100.0);
        assert_eq!(controller.mouse_sensitivity(), MOUSE_SENSITIVITY_MAX);
        controller.set_mouse_sensitivity(-5.0);
        assert_eq!(controller.mouse_sensitivity(), MOUSE_SENSITIVITY_MIN);
    }

    #[test]
    fn apply_look_delta_scales_the_resulting_yaw_change_by_mouse_sensitivity() {
        let mut low = FlyController::new();
        let mut high = FlyController::new();
        high.set_mouse_sensitivity(2.0);

        let mut camera_low = Camera3D::default();
        let mut camera_high = Camera3D::default();
        low.apply_look_delta(30.0, 5.0, &mut camera_low);
        high.apply_look_delta(30.0, 5.0, &mut camera_high);

        let yaw_delta_low = camera_low.yaw - Camera3D::default().yaw;
        let yaw_delta_high = camera_high.yaw - Camera3D::default().yaw;
        assert!(yaw_delta_low.abs() > 0.0);
        let ratio = yaw_delta_high / yaw_delta_low;
        assert!((ratio - 2.0).abs() < 1e-4, "2x mouse sensitivity must double the resulting yaw delta, got ratio {ratio}");
    }

    #[test]
    fn set_input_normalizes_a_diagonal_to_unit_length() {
        let mut controller = FlyController::new();
        controller.set_input(1.0, 1.0);
        let [h, v] = controller.input();
        assert!((h * h + v * v).sqrt() <= 1.0 + 1e-6, "a diagonal input must be clamped to unit length");
        assert!((h - v).abs() < 1e-6, "diagonal normalization must preserve the original direction");
    }

    #[test]
    fn stop_immediately_clears_keys_input_and_velocity() {
        let mut controller = FlyController::new();
        let mut camera = Camera3D::default();
        controller.set_key(KeyCode::W, true);
        converge(&mut controller, &mut camera, 1.0);
        assert!(controller.velocity()[1] > 0.0);

        controller.stop();
        assert_eq!(controller.input(), [0.0, 0.0]);
        assert_eq!(controller.velocity(), [0.0, 0.0]);

        let target_before = camera.target;
        controller.tick(1.0 / 60.0, &mut camera);
        assert_eq!(camera.target, target_before, "a tick after stop() with no held keys must not move the camera");
    }

    #[test]
    fn on_event_consumes_key_events_and_gated_pointer_delta_only() {
        let mut controller = FlyController::new();
        let mut camera = Camera3D::default();

        assert!(controller.on_event(&key_down(KeyCode::W), false, &mut camera));
        assert_eq!(controller.input(), [0.0, 1.0]);

        let camera_before = camera;
        assert!(!controller.on_event(&PlatformEvent::PointerDelta { dx: 10.0, dy: 0.0 }, false, &mut camera), "pointer delta must be ignored while mouse look is not active");
        assert_eq!(camera.yaw, camera_before.yaw);

        assert!(controller.on_event(&PlatformEvent::PointerDelta { dx: 10.0, dy: 0.0 }, true, &mut camera));
        assert_ne!(camera.yaw, camera_before.yaw, "pointer delta must apply free look once mouse look is active");

        assert!(controller.on_event(&key_up(KeyCode::W), false, &mut camera));
        assert_eq!(controller.input(), [0.0, 0.0]);

        assert!(!controller.on_event(&PlatformEvent::Scroll { dx: 0.0, dy: 1.0 }, true, &mut camera), "an unrelated event must not be consumed");
    }
}
