//! Builder pattern for toast animations
//!
//! Provides fluent API for customizing toast animation parameters.

use super::defaults::*;
use super::types::{StackDirection, ToastAnimation, ToastDirection};
use crate::animation::{Easing, Spring};

/// Builder for SlideFade toast animation
#[derive(Debug, Clone)]
pub struct SlideFadeBuilder {
    enter_duration_ms: u64,
    exit_duration_ms: u64,
    hold_duration_ms: u64,
    enter_easing: Easing,
    exit_easing: Easing,
    direction: ToastDirection,
    offset: f64,
}

impl SlideFadeBuilder {
    /// Create a new builder with default values
    pub fn new() -> Self {
        let defaults = SlideFadeDefaults::default();
        Self {
            enter_duration_ms: defaults.enter_duration_ms,
            exit_duration_ms: defaults.exit_duration_ms,
            hold_duration_ms: defaults.hold_duration_ms,
            enter_easing: defaults.enter_easing,
            exit_easing: defaults.exit_easing,
            direction: defaults.direction,
            offset: defaults.offset,
        }
    }

    pub fn enter_duration_ms(mut self, ms: u64) -> Self {
        self.enter_duration_ms = ms;
        self
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.exit_duration_ms = ms;
        self
    }

    pub fn hold_duration_ms(mut self, ms: u64) -> Self {
        self.hold_duration_ms = ms;
        self
    }

    pub fn enter_easing(mut self, easing: Easing) -> Self {
        self.enter_easing = easing;
        self
    }

    pub fn exit_easing(mut self, easing: Easing) -> Self {
        self.exit_easing = easing;
        self
    }

    pub fn direction(mut self, direction: ToastDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn offset(mut self, offset: f64) -> Self {
        self.offset = offset;
        self
    }

    pub fn build(self) -> ToastAnimation {
        ToastAnimation::SlideFade {
            enter_duration_ms: self.enter_duration_ms,
            exit_duration_ms: self.exit_duration_ms,
            hold_duration_ms: self.hold_duration_ms,
            enter_easing: self.enter_easing,
            exit_easing: self.exit_easing,
            direction: self.direction,
            offset: self.offset,
        }
    }
}

impl Default for SlideFadeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SpringBounce toast animation
#[derive(Debug, Clone)]
pub struct SpringBounceBuilder {
    enter_spring: Spring,
    exit_duration_ms: u64,
    exit_easing: Easing,
    hold_duration_ms: u64,
    direction: ToastDirection,
}

impl SpringBounceBuilder {
    pub fn new() -> Self {
        let defaults = SpringBounceDefaults::default();
        Self {
            enter_spring: defaults.enter_spring,
            exit_duration_ms: defaults.exit_duration_ms,
            exit_easing: defaults.exit_easing,
            hold_duration_ms: defaults.hold_duration_ms,
            direction: defaults.direction,
        }
    }

    pub fn enter_spring(mut self, spring: Spring) -> Self {
        self.enter_spring = spring;
        self
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.exit_duration_ms = ms;
        self
    }

    pub fn exit_easing(mut self, easing: Easing) -> Self {
        self.exit_easing = easing;
        self
    }

    pub fn hold_duration_ms(mut self, ms: u64) -> Self {
        self.hold_duration_ms = ms;
        self
    }

    pub fn direction(mut self, direction: ToastDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn build(self) -> ToastAnimation {
        ToastAnimation::SpringBounce {
            enter_spring: self.enter_spring,
            exit_duration_ms: self.exit_duration_ms,
            exit_easing: self.exit_easing,
            hold_duration_ms: self.hold_duration_ms,
            direction: self.direction,
        }
    }
}

impl Default for SpringBounceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for StackPush toast animation
#[derive(Debug, Clone)]
pub struct StackPushBuilder {
    push_duration_ms: u64,
    push_easing: Easing,
    gap: f64,
    scale_factor: f64,
    direction: StackDirection,
}

impl StackPushBuilder {
    pub fn new() -> Self {
        let defaults = StackPushDefaults::default();
        Self {
            push_duration_ms: defaults.push_duration_ms,
            push_easing: defaults.push_easing,
            gap: defaults.gap,
            scale_factor: defaults.scale_factor,
            direction: defaults.direction,
        }
    }

    pub fn push_duration_ms(mut self, ms: u64) -> Self {
        self.push_duration_ms = ms;
        self
    }

    pub fn push_easing(mut self, easing: Easing) -> Self {
        self.push_easing = easing;
        self
    }

    pub fn gap(mut self, gap: f64) -> Self {
        self.gap = gap;
        self
    }

    pub fn scale_factor(mut self, factor: f64) -> Self {
        self.scale_factor = factor;
        self
    }

    pub fn direction(mut self, direction: StackDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn build(self) -> ToastAnimation {
        ToastAnimation::StackPush {
            push_duration_ms: self.push_duration_ms,
            push_easing: self.push_easing,
            gap: self.gap,
            scale_factor: self.scale_factor,
            direction: self.direction,
        }
    }
}

impl Default for StackPushBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SwipeDismiss toast animation
#[derive(Debug, Clone)]
pub struct SwipeDismissBuilder {
    velocity_threshold: f64,
    distance_threshold: f64,
    friction: f64,
    spring_back_spring: Spring,
}

impl SwipeDismissBuilder {
    pub fn new() -> Self {
        let defaults = SwipeDismissDefaults::default();
        Self {
            velocity_threshold: defaults.velocity_threshold,
            distance_threshold: defaults.distance_threshold,
            friction: defaults.friction,
            spring_back_spring: defaults.spring_back_spring,
        }
    }

    pub fn velocity_threshold(mut self, threshold: f64) -> Self {
        self.velocity_threshold = threshold;
        self
    }

    pub fn distance_threshold(mut self, threshold: f64) -> Self {
        self.distance_threshold = threshold;
        self
    }

    pub fn friction(mut self, friction: f64) -> Self {
        self.friction = friction;
        self
    }

    pub fn spring_back_spring(mut self, spring: Spring) -> Self {
        self.spring_back_spring = spring;
        self
    }

    pub fn build(self) -> ToastAnimation {
        ToastAnimation::SwipeDismiss {
            velocity_threshold: self.velocity_threshold,
            distance_threshold: self.distance_threshold,
            friction: self.friction,
            spring_back_spring: self.spring_back_spring,
        }
    }
}

impl Default for SwipeDismissBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ScaleFade toast animation
#[derive(Debug, Clone)]
pub struct ScaleFadeBuilder {
    enter_duration_ms: u64,
    exit_duration_ms: u64,
    hold_duration_ms: u64,
    enter_easing: Easing,
    exit_easing: Easing,
    scale_from: f64,
    scale_to: f64,
}

impl ScaleFadeBuilder {
    pub fn new() -> Self {
        let defaults = ScaleFadeDefaults::default();
        Self {
            enter_duration_ms: defaults.enter_duration_ms,
            exit_duration_ms: defaults.exit_duration_ms,
            hold_duration_ms: defaults.hold_duration_ms,
            enter_easing: defaults.enter_easing,
            exit_easing: defaults.exit_easing,
            scale_from: defaults.scale_from,
            scale_to: defaults.scale_to,
        }
    }

    pub fn enter_duration_ms(mut self, ms: u64) -> Self {
        self.enter_duration_ms = ms;
        self
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.exit_duration_ms = ms;
        self
    }

    pub fn hold_duration_ms(mut self, ms: u64) -> Self {
        self.hold_duration_ms = ms;
        self
    }

    pub fn enter_easing(mut self, easing: Easing) -> Self {
        self.enter_easing = easing;
        self
    }

    pub fn exit_easing(mut self, easing: Easing) -> Self {
        self.exit_easing = easing;
        self
    }

    pub fn scale_from(mut self, scale: f64) -> Self {
        self.scale_from = scale;
        self
    }

    pub fn scale_to(mut self, scale: f64) -> Self {
        self.scale_to = scale;
        self
    }

    pub fn build(self) -> ToastAnimation {
        ToastAnimation::ScaleFade {
            enter_duration_ms: self.enter_duration_ms,
            exit_duration_ms: self.exit_duration_ms,
            hold_duration_ms: self.hold_duration_ms,
            enter_easing: self.enter_easing,
            exit_easing: self.exit_easing,
            scale_from: self.scale_from,
            scale_to: self.scale_to,
        }
    }
}

impl Default for ScaleFadeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for DropIn toast animation
#[derive(Debug, Clone)]
pub struct DropInBuilder {
    spring: Spring,
    exit_duration_ms: u64,
    exit_easing: Easing,
    hold_duration_ms: u64,
    drop_height: f64,
}

impl DropInBuilder {
    pub fn new() -> Self {
        let defaults = DropInDefaults::default();
        Self {
            spring: defaults.spring,
            exit_duration_ms: defaults.exit_duration_ms,
            exit_easing: defaults.exit_easing,
            hold_duration_ms: defaults.hold_duration_ms,
            drop_height: defaults.drop_height,
        }
    }

    pub fn spring(mut self, spring: Spring) -> Self {
        self.spring = spring;
        self
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.exit_duration_ms = ms;
        self
    }

    pub fn exit_easing(mut self, easing: Easing) -> Self {
        self.exit_easing = easing;
        self
    }

    pub fn hold_duration_ms(mut self, ms: u64) -> Self {
        self.hold_duration_ms = ms;
        self
    }

    pub fn drop_height(mut self, height: f64) -> Self {
        self.drop_height = height;
        self
    }

    pub fn build(self) -> ToastAnimation {
        ToastAnimation::DropIn {
            spring: self.spring,
            exit_duration_ms: self.exit_duration_ms,
            exit_easing: self.exit_easing,
            hold_duration_ms: self.hold_duration_ms,
            drop_height: self.drop_height,
        }
    }
}

impl Default for DropInBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ProgressBar toast animation
#[derive(Debug, Clone)]
pub struct ProgressBarBuilder {
    enter_duration_ms: u64,
    enter_easing: Easing,
    exit_duration_ms: u64,
    exit_easing: Easing,
    hold_duration_ms: u64,
    progress_easing: Easing,
    direction: ToastDirection,
}

impl ProgressBarBuilder {
    pub fn new() -> Self {
        let defaults = ProgressBarDefaults::default();
        Self {
            enter_duration_ms: defaults.enter_duration_ms,
            enter_easing: defaults.enter_easing,
            exit_duration_ms: defaults.exit_duration_ms,
            exit_easing: defaults.exit_easing,
            hold_duration_ms: defaults.hold_duration_ms,
            progress_easing: defaults.progress_easing,
            direction: defaults.direction,
        }
    }

    pub fn enter_duration_ms(mut self, ms: u64) -> Self {
        self.enter_duration_ms = ms;
        self
    }

    pub fn enter_easing(mut self, easing: Easing) -> Self {
        self.enter_easing = easing;
        self
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.exit_duration_ms = ms;
        self
    }

    pub fn exit_easing(mut self, easing: Easing) -> Self {
        self.exit_easing = easing;
        self
    }

    pub fn hold_duration_ms(mut self, ms: u64) -> Self {
        self.hold_duration_ms = ms;
        self
    }

    pub fn progress_easing(mut self, easing: Easing) -> Self {
        self.progress_easing = easing;
        self
    }

    pub fn direction(mut self, direction: ToastDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn build(self) -> ToastAnimation {
        ToastAnimation::ProgressBar {
            enter_duration_ms: self.enter_duration_ms,
            enter_easing: self.enter_easing,
            exit_duration_ms: self.exit_duration_ms,
            exit_easing: self.exit_easing,
            hold_duration_ms: self.hold_duration_ms,
            progress_easing: self.progress_easing,
            direction: self.direction,
        }
    }
}

impl Default for ProgressBarBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_fade_builder() {
        let anim = SlideFadeBuilder::new()
            .enter_duration_ms(500)
            .hold_duration_ms(4000)
            .direction(ToastDirection::Top)
            .build();

        assert_eq!(anim.total_duration_ms(), 500 + 4000 + 200); // default exit is 200ms
    }

    #[test]
    fn test_spring_bounce_builder() {
        let spring = Spring::new().stiffness(400.0).damping(30.0);
        let anim = SpringBounceBuilder::new()
            .enter_spring(spring)
            .hold_duration_ms(2000)
            .build();

        let duration = anim.total_duration_ms();
        assert!(duration > 2000);
    }

    #[test]
    fn test_stack_push_builder() {
        let anim = StackPushBuilder::new()
            .gap(20.0)
            .scale_factor(0.1)
            .direction(StackDirection::Down)
            .build();

        assert_eq!(anim.total_duration_ms(), 400); // default push duration
    }

    #[test]
    fn test_swipe_dismiss_builder() {
        let anim = SwipeDismissBuilder::new()
            .velocity_threshold(0.2)
            .distance_threshold(150.0)
            .build();

        assert_eq!(anim.total_duration_ms(), 0); // gesture-driven
    }

    #[test]
    fn test_scale_fade_builder() {
        let anim = ScaleFadeBuilder::new()
            .scale_from(0.5)
            .scale_to(1.2)
            .hold_duration_ms(5000)
            .build();

        assert_eq!(anim.total_duration_ms(), 200 + 5000 + 150); // defaults
    }

    #[test]
    fn test_drop_in_builder() {
        let anim = DropInBuilder::new()
            .drop_height(200.0)
            .hold_duration_ms(4000)
            .build();

        let duration = anim.total_duration_ms();
        assert!(duration > 4000);
    }

    #[test]
    fn test_progress_bar_builder() {
        let anim = ProgressBarBuilder::new()
            .hold_duration_ms(10000)
            .direction(ToastDirection::Left)
            .build();

        assert_eq!(anim.total_duration_ms(), 300 + 10000 + 300);
    }

    #[test]
    fn test_builder_defaults() {
        let _ = SlideFadeBuilder::default().build();
        let _ = SpringBounceBuilder::default().build();
        let _ = StackPushBuilder::default().build();
        let _ = SwipeDismissBuilder::default().build();
        let _ = ScaleFadeBuilder::default().build();
        let _ = DropInBuilder::default().build();
        let _ = ProgressBarBuilder::default().build();
    }
}
