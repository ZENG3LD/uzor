//! Toast animation type catalog
//!
//! Defines all toast/notification animation variants with their parameters.

use crate::{Decay, Easing, Spring, Timeline, Tween};
use std::time::Duration;

/// Catalog of toast/notification animation patterns
#[derive(Debug, Clone)]
pub enum ToastAnimation {
    /// Slide + fade from edge, hold, slide + fade out
    SlideFade {
        enter_duration_ms: u64,
        exit_duration_ms: u64,
        hold_duration_ms: u64,
        enter_easing: Easing,
        exit_easing: Easing,
        direction: ToastDirection,
        offset: f64, // pixels to slide from
    },

    /// Spring physics enter with bounce
    SpringBounce {
        enter_spring: Spring,
        exit_duration_ms: u64,
        exit_easing: Easing,
        hold_duration_ms: u64,
        direction: ToastDirection,
    },

    /// New toast pushes existing toasts up/down (stack compression)
    StackPush {
        push_duration_ms: u64,
        push_easing: Easing,
        gap: f64,             // pixels between toasts
        scale_factor: f64,    // scale reduction per stack position (e.g., 0.05 = 5%)
        direction: StackDirection,
    },

    /// Swipe-to-dismiss with velocity threshold
    SwipeDismiss {
        velocity_threshold: f64,  // units per millisecond
        distance_threshold: f64,  // pixels
        friction: f64,            // for decay animation
        spring_back_spring: Spring, // if swipe canceled
    },

    /// Scale from small + fade in
    ScaleFade {
        enter_duration_ms: u64,
        exit_duration_ms: u64,
        hold_duration_ms: u64,
        enter_easing: Easing,
        exit_easing: Easing,
        scale_from: f64,
        scale_to: f64,
    },

    /// Drops from top with slight bounce
    DropIn {
        spring: Spring,
        exit_duration_ms: u64,
        exit_easing: Easing,
        hold_duration_ms: u64,
        drop_height: f64, // pixels above final position
    },

    /// Toast with linear progress countdown bar
    ProgressBar {
        enter_duration_ms: u64,
        enter_easing: Easing,
        exit_duration_ms: u64,
        exit_easing: Easing,
        hold_duration_ms: u64,
        progress_easing: Easing, // typically Linear
        direction: ToastDirection,
    },
}

/// Direction for toast entry/exit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastDirection {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Direction for stack behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackDirection {
    Up,   // New toasts push upward
    Down, // New toasts push downward
}

impl ToastAnimation {
    /// Get enter timeline for the animation
    pub fn enter_timeline(&self) -> Timeline {
        match self {
            ToastAnimation::SlideFade {
                enter_duration_ms,
                enter_easing,
                ..
            } => {
                let tween = Tween::new(1.0, 0.0)
                    .duration(Duration::from_millis(*enter_duration_ms))
                    .easing(*enter_easing);

                let mut timeline = Timeline::new();
                timeline.add(tween.duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::SpringBounce {
                enter_spring,
                ..
            } => {
                let mut timeline = Timeline::new();
                let duration = Duration::from_secs_f64(enter_spring.estimated_duration());
                timeline.add(duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::StackPush {
                push_duration_ms,
                ..
            } => {
                let mut timeline = Timeline::new();
                timeline.add(
                    Duration::from_millis(*push_duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
                timeline
            }

            ToastAnimation::SwipeDismiss { .. } => {
                // Swipe is gesture-driven, no fixed enter timeline
                Timeline::new()
            }

            ToastAnimation::ScaleFade {
                enter_duration_ms,
                enter_easing,
                ..
            } => {
                let tween = Tween::new(1.0, 0.0)
                    .duration(Duration::from_millis(*enter_duration_ms))
                    .easing(*enter_easing);

                let mut timeline = Timeline::new();
                timeline.add(tween.duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::DropIn { spring, .. } => {
                let mut timeline = Timeline::new();
                let duration = Duration::from_secs_f64(spring.estimated_duration());
                timeline.add(duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::ProgressBar {
                enter_duration_ms,
                ..
            } => {
                let mut timeline = Timeline::new();
                timeline.add(
                    Duration::from_millis(*enter_duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
                timeline
            }
        }
    }

    /// Get exit timeline for the animation
    pub fn exit_timeline(&self) -> Timeline {
        match self {
            ToastAnimation::SlideFade {
                exit_duration_ms,
                exit_easing,
                ..
            } => {
                let tween = Tween::new(0.0, 1.0)
                    .duration(Duration::from_millis(*exit_duration_ms))
                    .easing(*exit_easing);

                let mut timeline = Timeline::new();
                timeline.add(tween.duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::SpringBounce {
                exit_duration_ms,
                exit_easing,
                ..
            } => {
                let tween = Tween::new(0.0, 1.0)
                    .duration(Duration::from_millis(*exit_duration_ms))
                    .easing(*exit_easing);

                let mut timeline = Timeline::new();
                timeline.add(tween.duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::StackPush {
                push_duration_ms,
                ..
            } => {
                let mut timeline = Timeline::new();
                timeline.add(
                    Duration::from_millis(*push_duration_ms),
                    crate::timeline::Position::Absolute(Duration::ZERO),
                );
                timeline
            }

            ToastAnimation::SwipeDismiss { friction, .. } => {
                // Exit via decay animation
                let mut timeline = Timeline::new();
                let decay = Decay::new(0.0).friction(*friction);
                let duration = Duration::from_secs_f64(decay.estimated_duration());
                timeline.add(duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::ScaleFade {
                exit_duration_ms,
                exit_easing,
                ..
            } => {
                let tween = Tween::new(0.0, 1.0)
                    .duration(Duration::from_millis(*exit_duration_ms))
                    .easing(*exit_easing);

                let mut timeline = Timeline::new();
                timeline.add(tween.duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::DropIn {
                exit_duration_ms,
                exit_easing,
                ..
            } => {
                let tween = Tween::new(0.0, 1.0)
                    .duration(Duration::from_millis(*exit_duration_ms))
                    .easing(*exit_easing);

                let mut timeline = Timeline::new();
                timeline.add(tween.duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }

            ToastAnimation::ProgressBar {
                exit_duration_ms,
                exit_easing,
                ..
            } => {
                let tween = Tween::new(0.0, 1.0)
                    .duration(Duration::from_millis(*exit_duration_ms))
                    .easing(*exit_easing);

                let mut timeline = Timeline::new();
                timeline.add(tween.duration, crate::timeline::Position::Absolute(Duration::ZERO));
                timeline
            }
        }
    }

    /// Get total duration (enter + hold + exit) in milliseconds
    pub fn total_duration_ms(&self) -> u64 {
        match self {
            ToastAnimation::SlideFade {
                enter_duration_ms,
                exit_duration_ms,
                hold_duration_ms,
                ..
            } => enter_duration_ms + hold_duration_ms + exit_duration_ms,

            ToastAnimation::SpringBounce {
                enter_spring,
                exit_duration_ms,
                hold_duration_ms,
                ..
            } => {
                (enter_spring.estimated_duration() * 1000.0) as u64
                    + hold_duration_ms
                    + exit_duration_ms
            }

            ToastAnimation::StackPush {
                push_duration_ms, ..
            } => *push_duration_ms,

            ToastAnimation::SwipeDismiss { .. } => {
                // Gesture-driven, no fixed duration
                0
            }

            ToastAnimation::ScaleFade {
                enter_duration_ms,
                exit_duration_ms,
                hold_duration_ms,
                ..
            } => enter_duration_ms + hold_duration_ms + exit_duration_ms,

            ToastAnimation::DropIn {
                spring,
                exit_duration_ms,
                hold_duration_ms,
                ..
            } => {
                (spring.estimated_duration() * 1000.0) as u64 + hold_duration_ms + exit_duration_ms
            }

            ToastAnimation::ProgressBar {
                enter_duration_ms,
                exit_duration_ms,
                hold_duration_ms,
                ..
            } => enter_duration_ms + hold_duration_ms + exit_duration_ms,
        }
    }

    /// Helper: calculate offset from direction
    #[allow(dead_code)]
    fn direction_offset(direction: ToastDirection, offset: f64) -> (f64, f64) {
        match direction {
            ToastDirection::Top => (0.0, -offset),
            ToastDirection::Bottom => (0.0, offset),
            ToastDirection::Left => (-offset, 0.0),
            ToastDirection::Right => (offset, 0.0),
            ToastDirection::TopLeft => (-offset, -offset),
            ToastDirection::TopRight => (offset, -offset),
            ToastDirection::BottomLeft => (-offset, offset),
            ToastDirection::BottomRight => (offset, offset),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_fade_duration() {
        let anim = ToastAnimation::SlideFade {
            enter_duration_ms: 300,
            exit_duration_ms: 200,
            hold_duration_ms: 3000,
            enter_easing: Easing::EASE_OUT,
            exit_easing: Easing::EASE_IN,
            direction: ToastDirection::Bottom,
            offset: 50.0,
        };

        assert_eq!(anim.total_duration_ms(), 3500);
    }

    #[test]
    fn test_spring_bounce_duration() {
        let spring = Spring::new().stiffness(350.0).damping(28.0);
        let anim = ToastAnimation::SpringBounce {
            enter_spring: spring,
            exit_duration_ms: 200,
            exit_easing: Easing::EASE_IN,
            hold_duration_ms: 5000,
            direction: ToastDirection::Bottom,
        };

        let duration = anim.total_duration_ms();
        assert!(duration > 5000); // At least hold + exit
        assert!(duration < 10000); // Should be reasonable
    }

    #[test]
    fn test_direction_offset() {
        let (x, y) = ToastAnimation::direction_offset(ToastDirection::Top, 100.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, -100.0);

        let (x, y) = ToastAnimation::direction_offset(ToastDirection::Right, 50.0);
        assert_eq!(x, 50.0);
        assert_eq!(y, 0.0);

        let (x, y) = ToastAnimation::direction_offset(ToastDirection::BottomLeft, 30.0);
        assert_eq!(x, -30.0);
        assert_eq!(y, 30.0);
    }

    #[test]
    fn test_timelines_creation() {
        let anim = ToastAnimation::ScaleFade {
            enter_duration_ms: 200,
            exit_duration_ms: 200,
            hold_duration_ms: 3000,
            enter_easing: Easing::EASE_OUT,
            exit_easing: Easing::EASE_IN,
            scale_from: 0.8,
            scale_to: 1.0,
        };

        let enter_tl = anim.enter_timeline();
        let exit_tl = anim.exit_timeline();

        assert_eq!(enter_tl.total_duration(), Duration::from_millis(200));
        assert_eq!(exit_tl.total_duration(), Duration::from_millis(200));
    }
}
