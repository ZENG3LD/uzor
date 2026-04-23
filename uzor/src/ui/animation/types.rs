//! Supporting types for animation coordination

use crate::types::WidgetId;
use crate::animation::{Decay, Easing, Spring};

/// Identifies a specific animated property on a specific widget
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AnimationKey {
    pub widget_id: WidgetId,
    pub property: String,
}

impl AnimationKey {
    pub fn new(widget_id: WidgetId, property: impl Into<String>) -> Self {
        Self {
            widget_id,
            property: property.into(),
        }
    }
}

/// What drives the animation
#[derive(Debug, Clone)]
pub enum AnimationDriver {
    Tween {
        from: f64,
        to: f64,
        start_time: f64,
        duration: f64,
        easing: Easing,
    },
    Spring {
        spring: Spring,
        start_time: f64,
        target: f64,
    },
    Decay {
        decay: Decay,
        start_time: f64,
        initial_value: f64,
    },
}

/// Active animation state
#[derive(Debug, Clone)]
pub struct ActiveAnimation {
    pub driver: AnimationDriver,
    pub current_value: f64,
    pub completed: bool,
}

impl ActiveAnimation {
    /// Create a new tween animation
    pub fn tween(from: f64, to: f64, start_time: f64, duration: f64, easing: Easing) -> Self {
        Self {
            driver: AnimationDriver::Tween {
                from,
                to,
                start_time,
                duration,
                easing,
            },
            current_value: from,
            completed: false,
        }
    }

    /// Create a new spring animation
    pub fn spring(spring: Spring, start_time: f64, target: f64) -> Self {
        Self {
            driver: AnimationDriver::Spring {
                spring,
                start_time,
                target,
            },
            current_value: 1.0, // Spring starts at displacement of 1.0
            completed: false,
        }
    }

    /// Create a new decay animation
    pub fn decay(decay: Decay, start_time: f64, initial_value: f64) -> Self {
        Self {
            driver: AnimationDriver::Decay {
                decay,
                start_time,
                initial_value,
            },
            current_value: initial_value,
            completed: false,
        }
    }

    /// Update animation state at the given time
    pub fn update(&mut self, time_secs: f64) {
        match &self.driver {
            AnimationDriver::Tween {
                from,
                to,
                start_time,
                duration,
                easing,
            } => {
                let elapsed = time_secs - start_time;
                if elapsed >= *duration {
                    self.current_value = *to;
                    self.completed = true;
                } else {
                    let t = (elapsed / duration).clamp(0.0, 1.0);
                    let eased_t = easing.ease(t);
                    self.current_value = from + (to - from) * eased_t;
                    self.completed = false;
                }
            }
            AnimationDriver::Spring {
                spring,
                start_time,
                target,
            } => {
                let elapsed = time_secs - start_time;
                let (displacement, _velocity) = spring.evaluate(elapsed);

                // Spring returns displacement from target (1.0 at start, 0.0 at rest)
                // Convert to actual value: value = target - displacement
                self.current_value = target - displacement;

                // Check if spring is at rest
                self.completed = spring.is_at_rest(elapsed);
            }
            AnimationDriver::Decay {
                decay,
                start_time,
                initial_value,
            } => {
                let elapsed = time_secs - start_time;
                let (position, _velocity) = decay.evaluate(elapsed);

                // Decay returns position offset from start
                self.current_value = initial_value + position;

                // Check if decay is at rest
                self.completed = decay.is_at_rest(elapsed);
            }
        }
    }
}
