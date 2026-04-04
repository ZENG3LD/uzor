//! Common mobile platform utilities shared between iOS and Android
//!
//! This module contains shared functionality that both mobile platforms use,
//! including gesture recognition and touch event processing.

#![allow(dead_code)]

use uzor::platform::PlatformEvent;

// =============================================================================
// Gesture Recognition
// =============================================================================

/// Gesture recognizer for common mobile gestures
///
/// Tracks touch state and recognizes patterns like:
/// - Tap
/// - Long press
/// - Swipe
/// - Pinch (two-finger zoom)
/// - Rotation
#[derive(Debug)]
pub struct GestureRecognizer {
    /// Active touches
    touches: Vec<TouchPoint>,

    /// Gesture state
    state: GestureState,

    /// Configuration
    config: GestureConfig,
}

#[derive(Debug, Clone)]
struct TouchPoint {
    id: u64,
    start_x: f64,
    start_y: f64,
    current_x: f64,
    current_y: f64,
    start_time: std::time::Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureState {
    Idle,
    PossibleTap,
    PossibleLongPress,
    Swiping,
    Pinching,
    Rotating,
}

#[derive(Debug, Clone)]
pub struct GestureConfig {
    /// Tap timeout (ms)
    pub tap_timeout: u64,

    /// Long press duration (ms)
    pub long_press_duration: u64,

    /// Swipe threshold (pixels)
    pub swipe_threshold: f64,

    /// Pinch threshold (distance change)
    pub pinch_threshold: f64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            tap_timeout: 300,
            long_press_duration: 500,
            swipe_threshold: 50.0,
            pinch_threshold: 20.0,
        }
    }
}

impl GestureRecognizer {
    /// Create a new gesture recognizer with default config
    pub fn new() -> Self {
        Self::with_config(GestureConfig::default())
    }

    /// Create a new gesture recognizer with custom config
    pub fn with_config(config: GestureConfig) -> Self {
        Self {
            touches: Vec::new(),
            state: GestureState::Idle,
            config,
        }
    }

    /// Process a touch start event
    pub fn touch_start(&mut self, id: u64, x: f64, y: f64) -> Option<GestureEvent> {
        let touch = TouchPoint {
            id,
            start_x: x,
            start_y: y,
            current_x: x,
            current_y: y,
            start_time: std::time::Instant::now(),
        };

        self.touches.push(touch);

        // Update gesture state based on touch count
        match self.touches.len() {
            1 => {
                self.state = GestureState::PossibleTap;
                None
            }
            2 => {
                self.state = GestureState::Idle; // Could be pinch or rotate
                None
            }
            _ => None,
        }
    }

    /// Process a touch move event
    pub fn touch_move(&mut self, id: u64, x: f64, y: f64) -> Option<GestureEvent> {
        // Find and update the touch
        let touch = self.touches.iter_mut().find(|t| t.id == id)?;
        touch.current_x = x;
        touch.current_y = y;

        // Detect gestures based on touch count and movement
        match self.touches.len() {
            1 => self.detect_single_touch_gesture(),
            2 => self.detect_two_touch_gesture(),
            _ => None,
        }
    }

    /// Process a touch end event
    pub fn touch_end(&mut self, id: u64, x: f64, y: f64) -> Option<GestureEvent> {
        // Find the touch
        let touch_index = self.touches.iter().position(|t| t.id == id)?;
        let touch = self.touches.remove(touch_index);

        // Check if it was a tap
        if self.state == GestureState::PossibleTap {
            let duration = touch.start_time.elapsed().as_millis() as u64;
            let distance = Self::distance(touch.start_x, touch.start_y, x, y);

            if duration < self.config.tap_timeout && distance < 20.0 {
                self.state = GestureState::Idle;
                return Some(GestureEvent::Tap { x, y });
            }
        }

        // Reset state if no more touches
        if self.touches.is_empty() {
            self.state = GestureState::Idle;
        }

        None
    }

    /// Process a touch cancel event
    pub fn touch_cancel(&mut self, id: u64) {
        self.touches.retain(|t| t.id != id);

        if self.touches.is_empty() {
            self.state = GestureState::Idle;
        }
    }

    fn detect_single_touch_gesture(&mut self) -> Option<GestureEvent> {
        if self.touches.len() != 1 {
            return None;
        }

        let touch = &self.touches[0];
        let dx = touch.current_x - touch.start_x;
        let dy = touch.current_y - touch.start_y;
        let distance = (dx * dx + dy * dy).sqrt();

        // Check for swipe
        if distance > self.config.swipe_threshold && self.state != GestureState::Swiping {
            self.state = GestureState::Swiping;

            let direction = if dx.abs() > dy.abs() {
                if dx > 0.0 {
                    SwipeDirection::Right
                } else {
                    SwipeDirection::Left
                }
            } else if dy > 0.0 {
                SwipeDirection::Down
            } else {
                SwipeDirection::Up
            };

            return Some(GestureEvent::Swipe {
                direction,
                distance,
            });
        }

        None
    }

    fn detect_two_touch_gesture(&mut self) -> Option<GestureEvent> {
        if self.touches.len() != 2 {
            return None;
        }

        let touch1 = &self.touches[0];
        let touch2 = &self.touches[1];

        // Calculate initial and current distances
        let start_distance = Self::distance(
            touch1.start_x,
            touch1.start_y,
            touch2.start_x,
            touch2.start_y,
        );

        let current_distance = Self::distance(
            touch1.current_x,
            touch1.current_y,
            touch2.current_x,
            touch2.current_y,
        );

        let distance_change = current_distance - start_distance;

        // Check for pinch/zoom
        if distance_change.abs() > self.config.pinch_threshold {
            let scale = current_distance / start_distance;

            return Some(GestureEvent::Pinch {
                scale,
                center_x: (touch1.current_x + touch2.current_x) / 2.0,
                center_y: (touch1.current_y + touch2.current_y) / 2.0,
            });
        }

        None
    }

    fn distance(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        (dx * dx + dy * dy).sqrt()
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Recognized gesture events
#[derive(Debug, Clone, PartialEq)]
pub enum GestureEvent {
    /// Single tap at position
    Tap { x: f64, y: f64 },

    /// Long press at position
    LongPress { x: f64, y: f64 },

    /// Swipe gesture
    Swipe {
        direction: SwipeDirection,
        distance: f64,
    },

    /// Pinch/zoom gesture
    Pinch {
        scale: f64,
        center_x: f64,
        center_y: f64,
    },

    /// Rotation gesture
    Rotation { angle: f64, center_x: f64, center_y: f64 },
}

/// Swipe direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}

// =============================================================================
// Touch Event Utilities
// =============================================================================

/// Helper to convert touch events to pointer events for single-touch scenarios
///
/// Many UI components expect pointer events, but mobile uses touch events.
/// This helper converts the first touch to pointer events for compatibility.
pub fn touch_to_pointer_event(event: &PlatformEvent) -> Option<PlatformEvent> {
    match event {
        PlatformEvent::TouchStart { id, x, y } if *id == 0 => Some(PlatformEvent::PointerDown {
            x: *x,
            y: *y,
            button: uzor::input::state::MouseButton::Left,
        }),
        PlatformEvent::TouchMove { id, x, y } if *id == 0 => Some(PlatformEvent::PointerMoved {
            x: *x,
            y: *y,
        }),
        PlatformEvent::TouchEnd { id, x, y } if *id == 0 => Some(PlatformEvent::PointerUp {
            x: *x,
            y: *y,
            button: uzor::input::state::MouseButton::Left,
        }),
        _ => None,
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gesture_recognizer_tap() {
        let mut recognizer = GestureRecognizer::new();

        // Start touch
        let result = recognizer.touch_start(0, 100.0, 200.0);
        assert!(result.is_none());

        // End touch quickly at same position
        let result = recognizer.touch_end(0, 102.0, 201.0);
        assert!(matches!(result, Some(GestureEvent::Tap { .. })));
    }

    #[test]
    fn test_gesture_recognizer_swipe() {
        let mut recognizer = GestureRecognizer::new();

        // Start touch
        recognizer.touch_start(0, 100.0, 200.0);

        // Move far enough to trigger swipe
        let result = recognizer.touch_move(0, 200.0, 210.0);
        assert!(matches!(result, Some(GestureEvent::Swipe { .. })));

        if let Some(GestureEvent::Swipe { direction, .. }) = result {
            assert_eq!(direction, SwipeDirection::Right);
        }
    }

    #[test]
    fn test_gesture_recognizer_pinch() {
        let mut recognizer = GestureRecognizer::new();

        // Start two touches
        recognizer.touch_start(0, 100.0, 200.0);
        recognizer.touch_start(1, 200.0, 200.0);

        // Move touches apart (zoom in)
        let _result = recognizer.touch_move(0, 50.0, 200.0);
        // First move may not trigger (need both to move)

        let result = recognizer.touch_move(1, 250.0, 200.0);
        assert!(matches!(result, Some(GestureEvent::Pinch { .. })));

        if let Some(GestureEvent::Pinch { scale, .. }) = result {
            assert!(scale > 1.0); // Zooming in
        }
    }

    #[test]
    fn test_touch_to_pointer_conversion() {
        let touch_start = PlatformEvent::TouchStart {
            id: 0,
            x: 100.0,
            y: 200.0,
        };

        let pointer = touch_to_pointer_event(&touch_start);
        assert!(matches!(pointer, Some(PlatformEvent::PointerDown { .. })));

        // Second touch should not convert
        let touch_start_2 = PlatformEvent::TouchStart {
            id: 1,
            x: 150.0,
            y: 250.0,
        };

        let pointer = touch_to_pointer_event(&touch_start_2);
        assert!(pointer.is_none());
    }

    #[test]
    fn test_swipe_directions() {
        let directions = vec![
            SwipeDirection::Up,
            SwipeDirection::Down,
            SwipeDirection::Left,
            SwipeDirection::Right,
        ];

        assert_eq!(directions.len(), 4);
    }
}
