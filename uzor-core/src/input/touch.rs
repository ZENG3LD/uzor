//! Multi-touch gesture state and recognition
//!
//! This module provides touch tracking and gesture recognition for platforms
//! that support multi-touch input (tablets, touchscreens, trackpads).

use std::collections::HashMap;

/// Unique identifier for a touch point
pub type TouchId = u64;

/// Single touch point state
#[derive(Clone, Copy, Debug)]
pub struct Touch {
    /// Unique identifier for this touch
    pub id: TouchId,

    /// Current position in screen coordinates
    pub pos: (f64, f64),

    /// Previous position (for calculating delta)
    pub prev_pos: (f64, f64),

    /// Force/pressure (0.0 to 1.0, optional depending on hardware)
    pub force: Option<f64>,

    /// Time when this touch started (in seconds)
    pub start_time: f64,
}

impl Touch {
    /// Create a new touch point
    pub fn new(id: TouchId, x: f64, y: f64, time: f64) -> Self {
        Self {
            id,
            pos: (x, y),
            prev_pos: (x, y),
            force: None,
            start_time: time,
        }
    }

    /// Update position
    pub fn update_pos(&mut self, x: f64, y: f64) {
        self.prev_pos = self.pos;
        self.pos = (x, y);
    }

    /// Get movement delta since last frame
    pub fn delta(&self) -> (f64, f64) {
        (self.pos.0 - self.prev_pos.0, self.pos.1 - self.prev_pos.1)
    }

    /// Get distance from another touch point
    pub fn distance_to(&self, other: &Touch) -> f64 {
        let dx = self.pos.0 - other.pos.0;
        let dy = self.pos.1 - other.pos.1;
        (dx * dx + dy * dy).sqrt()
    }

    /// Get angle to another touch point (in radians)
    pub fn angle_to(&self, other: &Touch) -> f64 {
        let dx = other.pos.0 - self.pos.0;
        let dy = other.pos.1 - self.pos.1;
        dy.atan2(dx)
    }
}

/// Multi-touch state tracker
#[derive(Clone, Debug, Default)]
pub struct TouchState {
    /// Active touch points by ID
    touches: HashMap<TouchId, Touch>,

    /// Previous touch count (for detecting touch count changes)
    prev_touch_count: usize,

    /// Pinch gesture: distance delta between two touches
    pub pinch_delta: Option<f64>,

    /// Previous distance for pinch calculation
    prev_pinch_distance: Option<f64>,

    /// Rotation gesture: angle delta between two touches (radians)
    pub rotation_delta: Option<f64>,

    /// Previous angle for rotation calculation
    prev_rotation_angle: Option<f64>,

    /// Two-finger pan: average movement of two touches
    pub pan_delta: Option<(f64, f64)>,

    /// Primary touch ID (first touch that went down)
    primary_id: Option<TouchId>,
}

impl TouchState {
    /// Create new empty touch state
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a touch point
    pub fn update_touch(&mut self, id: TouchId, x: f64, y: f64, time: f64, force: Option<f64>) {
        if let Some(touch) = self.touches.get_mut(&id) {
            touch.update_pos(x, y);
            touch.force = force;
        } else {
            let mut touch = Touch::new(id, x, y, time);
            touch.force = force;
            self.touches.insert(id, touch);

            // Set as primary if this is the first touch
            if self.primary_id.is_none() {
                self.primary_id = Some(id);
            }
        }

        self.update_gestures();
    }

    /// Start a new touch point (convenience wrapper)
    pub fn touch_start(&mut self, id: TouchId, x: f64, y: f64, time: f64, force: Option<f64>) {
        self.update_touch(id, x, y, time, force);
    }

    /// Move an existing touch point (convenience wrapper)
    pub fn touch_move(&mut self, id: TouchId, x: f64, y: f64, time: f64, force: Option<f64>) {
        self.update_touch(id, x, y, time, force);
    }

    /// End a touch point (convenience wrapper)
    pub fn touch_end(&mut self, id: TouchId) {
        self.remove_touch(id);
    }

    /// Cancel a touch point (convenience wrapper for touch_end)
    pub fn touch_cancel(&mut self, id: TouchId) {
        self.remove_touch(id);
    }

    /// Remove a touch point
    pub fn remove_touch(&mut self, id: TouchId) {
        self.touches.remove(&id);

        // Clear primary if it was removed
        if self.primary_id == Some(id) {
            self.primary_id = self.touches.keys().next().copied();
        }

        self.update_gestures();
    }

    /// Clear all touches
    pub fn clear(&mut self) {
        self.touches.clear();
        self.primary_id = None;
        self.clear_deltas();
    }

    /// Clear frame-specific gesture deltas
    pub fn clear_deltas(&mut self) {
        self.pinch_delta = None;
        self.rotation_delta = None;
        self.pan_delta = None;
        self.prev_touch_count = self.touches.len();
    }

    /// Update gesture calculations based on current touches
    fn update_gestures(&mut self) {
        let touch_count = self.touches.len();

        // Only process gestures when we have exactly 2 touches
        if touch_count == 2 {
            let touches: Vec<&Touch> = self.touches.values().collect();
            let t1 = touches[0];
            let t2 = touches[1];

            // Calculate pinch (distance change)
            let current_distance = t1.distance_to(t2);
            if let Some(prev_distance) = self.prev_pinch_distance {
                self.pinch_delta = Some(current_distance - prev_distance);
            }
            self.prev_pinch_distance = Some(current_distance);

            // Calculate rotation (angle change)
            let current_angle = t1.angle_to(t2);
            if let Some(prev_angle) = self.prev_rotation_angle {
                let mut angle_delta = current_angle - prev_angle;

                // Normalize angle delta to [-π, π]
                while angle_delta > std::f64::consts::PI {
                    angle_delta -= 2.0 * std::f64::consts::PI;
                }
                while angle_delta < -std::f64::consts::PI {
                    angle_delta += 2.0 * std::f64::consts::PI;
                }

                self.rotation_delta = Some(angle_delta);
            }
            self.prev_rotation_angle = Some(current_angle);

            // Calculate pan (average movement)
            let delta1 = t1.delta();
            let delta2 = t2.delta();
            let avg_delta = (
                (delta1.0 + delta2.0) / 2.0,
                (delta1.1 + delta2.1) / 2.0,
            );

            // Only set pan if there's significant movement
            if avg_delta.0.abs() > 0.1 || avg_delta.1.abs() > 0.1 {
                self.pan_delta = Some(avg_delta);
            }
        } else {
            // Not exactly 2 touches - clear multi-touch gestures
            if self.prev_touch_count == 2 {
                // Just transitioned away from 2 touches
                self.prev_pinch_distance = None;
                self.prev_rotation_angle = None;
            }
        }
    }

    /// Get the primary touch (first touch that went down)
    pub fn primary_touch(&self) -> Option<&Touch> {
        self.primary_id.and_then(|id| self.touches.get(&id))
    }

    /// Get touch by ID
    pub fn get_touch(&self, id: TouchId) -> Option<&Touch> {
        self.touches.get(&id)
    }

    /// Get all active touches
    pub fn touches(&self) -> impl Iterator<Item = &Touch> {
        self.touches.values()
    }

    /// Get number of active touches
    pub fn touch_count(&self) -> usize {
        self.touches.len()
    }

    /// Check if any touches are active
    pub fn has_touches(&self) -> bool {
        !self.touches.is_empty()
    }

    /// Get centroid (average position) of all touches
    pub fn centroid(&self) -> Option<(f64, f64)> {
        if self.touches.is_empty() {
            return None;
        }

        let count = self.touches.len() as f64;
        let sum = self.touches.values().fold((0.0, 0.0), |acc, touch| {
            (acc.0 + touch.pos.0, acc.1 + touch.pos.1)
        });

        Some((sum.0 / count, sum.1 / count))
    }

    /// Check if this is a two-finger gesture
    pub fn is_two_finger_gesture(&self) -> bool {
        self.touches.len() == 2
    }

    /// Check if currently pinching
    pub fn is_pinching(&self) -> bool {
        self.pinch_delta.is_some() && self.pinch_delta.unwrap().abs() > 0.1
    }

    /// Check if currently rotating
    pub fn is_rotating(&self) -> bool {
        self.rotation_delta.is_some() && self.rotation_delta.unwrap().abs() > 0.01
    }

    /// Check if currently panning with two fingers
    pub fn is_two_finger_panning(&self) -> bool {
        self.pan_delta.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_creation() {
        let touch = Touch::new(1, 100.0, 200.0, 0.0);
        assert_eq!(touch.id, 1);
        assert_eq!(touch.pos, (100.0, 200.0));
        assert_eq!(touch.prev_pos, (100.0, 200.0));
        assert_eq!(touch.delta(), (0.0, 0.0));
    }

    #[test]
    fn test_touch_update() {
        let mut touch = Touch::new(1, 100.0, 200.0, 0.0);
        touch.update_pos(150.0, 250.0);

        assert_eq!(touch.pos, (150.0, 250.0));
        assert_eq!(touch.prev_pos, (100.0, 200.0));
        assert_eq!(touch.delta(), (50.0, 50.0));
    }

    #[test]
    fn test_touch_distance() {
        let t1 = Touch::new(1, 0.0, 0.0, 0.0);
        let t2 = Touch::new(2, 3.0, 4.0, 0.0);

        assert_eq!(t1.distance_to(&t2), 5.0); // 3-4-5 triangle
    }

    #[test]
    fn test_touch_angle() {
        let t1 = Touch::new(1, 0.0, 0.0, 0.0);
        let t2 = Touch::new(2, 1.0, 0.0, 0.0);

        assert_eq!(t1.angle_to(&t2), 0.0); // Pointing right

        let t3 = Touch::new(3, 0.0, 1.0, 0.0);
        assert!((t1.angle_to(&t3) - std::f64::consts::PI / 2.0).abs() < 0.001); // Pointing up
    }

    #[test]
    fn test_touch_state_add_remove() {
        let mut state = TouchState::new();
        assert_eq!(state.touch_count(), 0);
        assert!(!state.has_touches());

        state.update_touch(1, 100.0, 200.0, 0.0, None);
        assert_eq!(state.touch_count(), 1);
        assert!(state.has_touches());
        assert_eq!(state.primary_touch().unwrap().id, 1);

        state.update_touch(2, 300.0, 400.0, 0.0, None);
        assert_eq!(state.touch_count(), 2);

        state.remove_touch(1);
        assert_eq!(state.touch_count(), 1);
        assert_eq!(state.primary_touch().unwrap().id, 2); // Primary switches to remaining touch

        state.clear();
        assert_eq!(state.touch_count(), 0);
        assert!(!state.has_touches());
    }

    #[test]
    fn test_centroid() {
        let mut state = TouchState::new();

        state.update_touch(1, 0.0, 0.0, 0.0, None);
        assert_eq!(state.centroid(), Some((0.0, 0.0)));

        state.update_touch(2, 100.0, 100.0, 0.0, None);
        assert_eq!(state.centroid(), Some((50.0, 50.0)));

        state.update_touch(3, 200.0, 200.0, 0.0, None);
        assert_eq!(state.centroid(), Some((100.0, 100.0)));
    }

    #[test]
    fn test_pinch_gesture() {
        let mut state = TouchState::new();

        // Start with two touches 100 units apart
        state.update_touch(1, 0.0, 0.0, 0.0, None);
        state.update_touch(2, 100.0, 0.0, 0.0, None);
        assert!(state.is_two_finger_gesture());

        // Initial pinch_delta is None (no previous distance)
        assert!(state.pinch_delta.is_none());

        state.clear_deltas();

        // Move touches closer (pinch in)
        state.update_touch(1, 10.0, 0.0, 0.1, None);
        state.update_touch(2, 90.0, 0.0, 0.1, None);

        // Should detect pinch
        assert!(state.pinch_delta.is_some());
        let delta = state.pinch_delta.unwrap();
        assert!(delta < 0.0); // Distance decreased
    }

    #[test]
    fn test_rotation_gesture() {
        let mut state = TouchState::new();

        // Start horizontal
        state.update_touch(1, 0.0, 0.0, 0.0, None);
        state.update_touch(2, 100.0, 0.0, 0.0, None);

        state.clear_deltas();

        // Rotate to vertical (90 degrees)
        state.update_touch(1, 0.0, 0.0, 0.1, None);
        state.update_touch(2, 0.0, 100.0, 0.1, None);

        assert!(state.rotation_delta.is_some());
        let rotation = state.rotation_delta.unwrap();

        // Should be approximately π/2 radians (90 degrees)
        assert!((rotation - std::f64::consts::PI / 2.0).abs() < 0.1);
    }

    #[test]
    fn test_pan_gesture() {
        let mut state = TouchState::new();

        state.update_touch(1, 0.0, 0.0, 0.0, None);
        state.update_touch(2, 100.0, 0.0, 0.0, None);

        state.clear_deltas();

        // Move both touches right by 10
        state.update_touch(1, 10.0, 0.0, 0.1, None);
        state.update_touch(2, 110.0, 0.0, 0.1, None);

        assert!(state.pan_delta.is_some());
        let (dx, dy) = state.pan_delta.unwrap();
        assert!((dx - 10.0).abs() < 0.1);
        assert!(dy.abs() < 0.1);
    }

    #[test]
    fn test_clear_deltas() {
        let mut state = TouchState::new();

        state.update_touch(1, 0.0, 0.0, 0.0, None);
        state.update_touch(2, 100.0, 0.0, 0.0, None);

        // Generate some deltas
        state.update_touch(1, 10.0, 0.0, 0.1, None);
        state.update_touch(2, 90.0, 0.0, 0.1, None);

        // Clear deltas
        state.clear_deltas();

        assert!(state.pinch_delta.is_none());
        assert!(state.rotation_delta.is_none());
        assert!(state.pan_delta.is_none());
    }
}
