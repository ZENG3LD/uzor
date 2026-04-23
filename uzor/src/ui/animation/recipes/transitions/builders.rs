//! Builder patterns for transition animations
//!
//! Fluent API for customizing transition parameters.

use super::defaults::*;
use super::types::{SlideDirection, TransitionAnimation};
use crate::animation::Easing;

/// Builder for Shared Axis X transition
#[derive(Debug, Clone)]
pub struct SharedAxisXBuilder {
    params: SharedAxisDefaults,
}

impl SharedAxisXBuilder {
    pub fn new() -> Self {
        Self {
            params: SharedAxisDefaults::default(),
        }
    }

    pub fn enter_duration_ms(mut self, ms: u64) -> Self {
        self.params.enter_duration_ms = ms;
        self
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.params.exit_duration_ms = ms;
        self
    }

    pub fn overlap_ms(mut self, ms: u64) -> Self {
        self.params.overlap_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn distance(mut self, distance: f64) -> Self {
        self.params.distance = distance;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::SharedAxisX {
            enter_duration_ms: self.params.enter_duration_ms,
            exit_duration_ms: self.params.exit_duration_ms,
            overlap_ms: self.params.overlap_ms,
            easing: self.params.easing,
            distance: self.params.distance,
        }
    }
}

impl Default for SharedAxisXBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for Shared Axis Y transition
#[derive(Debug, Clone)]
pub struct SharedAxisYBuilder {
    params: SharedAxisDefaults,
}

impl SharedAxisYBuilder {
    pub fn new() -> Self {
        Self {
            params: SharedAxisDefaults::default(),
        }
    }

    pub fn enter_duration_ms(mut self, ms: u64) -> Self {
        self.params.enter_duration_ms = ms;
        self
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.params.exit_duration_ms = ms;
        self
    }

    pub fn overlap_ms(mut self, ms: u64) -> Self {
        self.params.overlap_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn distance(mut self, distance: f64) -> Self {
        self.params.distance = distance;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::SharedAxisY {
            enter_duration_ms: self.params.enter_duration_ms,
            exit_duration_ms: self.params.exit_duration_ms,
            overlap_ms: self.params.overlap_ms,
            easing: self.params.easing,
            distance: self.params.distance,
        }
    }
}

impl Default for SharedAxisYBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for Fade Through transition
#[derive(Debug, Clone)]
pub struct FadeThroughBuilder {
    params: FadeThroughDefaults,
}

impl FadeThroughBuilder {
    pub fn new() -> Self {
        Self {
            params: FadeThroughDefaults::default(),
        }
    }

    pub fn exit_duration_ms(mut self, ms: u64) -> Self {
        self.params.exit_duration_ms = ms;
        self
    }

    pub fn enter_duration_ms(mut self, ms: u64) -> Self {
        self.params.enter_duration_ms = ms;
        self
    }

    pub fn exit_easing(mut self, easing: Easing) -> Self {
        self.params.exit_easing = easing;
        self
    }

    pub fn enter_easing(mut self, easing: Easing) -> Self {
        self.params.enter_easing = easing;
        self
    }

    pub fn enter_scale_from(mut self, scale: f64) -> Self {
        self.params.enter_scale_from = scale;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::FadeThrough {
            exit_duration_ms: self.params.exit_duration_ms,
            enter_duration_ms: self.params.enter_duration_ms,
            exit_easing: self.params.exit_easing,
            enter_easing: self.params.enter_easing,
            enter_scale_from: self.params.enter_scale_from,
        }
    }
}

impl Default for FadeThroughBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for CrossFade transition
#[derive(Debug, Clone)]
pub struct CrossFadeBuilder {
    params: CrossFadeDefaults,
}

impl CrossFadeBuilder {
    pub fn new() -> Self {
        Self {
            params: CrossFadeDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.params.duration_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::CrossFade {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
        }
    }
}

impl Default for CrossFadeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for Push transition
#[derive(Debug, Clone)]
pub struct PushBuilder {
    params: PushDefaults,
}

impl PushBuilder {
    pub fn new() -> Self {
        Self {
            params: PushDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.params.duration_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn old_page_offset(mut self, offset: f64) -> Self {
        self.params.old_page_offset = offset;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::PushLeft {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            old_page_offset: self.params.old_page_offset,
        }
    }
}

impl Default for PushBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SlideOver transition
#[derive(Debug, Clone)]
pub struct SlideOverBuilder {
    params: SlideOverDefaults,
}

impl SlideOverBuilder {
    pub fn new() -> Self {
        Self {
            params: SlideOverDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.params.duration_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn direction(mut self, direction: SlideDirection) -> Self {
        self.params.direction = direction;
        self
    }

    pub fn enter_scale(mut self, scale: f64) -> Self {
        self.params.enter_scale = scale;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::SlideOver {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            direction: self.params.direction,
            enter_scale: self.params.enter_scale,
        }
    }
}

impl Default for SlideOverBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for Zoom transition
#[derive(Debug, Clone)]
pub struct ZoomBuilder {
    params: ZoomDefaults,
}

impl ZoomBuilder {
    pub fn new() -> Self {
        Self {
            params: ZoomDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.params.duration_ms = ms;
        self
    }

    pub fn old_scale_to(mut self, scale: f64) -> Self {
        self.params.old_scale_to = scale;
        self
    }

    pub fn new_scale_from(mut self, scale: f64) -> Self {
        self.params.new_scale_from = scale;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::ZoomIn {
            duration_ms: self.params.duration_ms,
            old_scale_to: self.params.old_scale_to,
            new_scale_from: self.params.new_scale_from,
            easing: self.params.easing,
        }
    }
}

impl Default for ZoomBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for CircleReveal transition
#[derive(Debug, Clone)]
pub struct CircleRevealBuilder {
    params: CircleRevealDefaults,
}

impl CircleRevealBuilder {
    pub fn new() -> Self {
        Self {
            params: CircleRevealDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.params.duration_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn origin(mut self, x: f64, y: f64) -> Self {
        self.params.origin_x = x;
        self.params.origin_y = y;
        self
    }

    pub fn origin_x(mut self, x: f64) -> Self {
        self.params.origin_x = x;
        self
    }

    pub fn origin_y(mut self, y: f64) -> Self {
        self.params.origin_y = y;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::CircleReveal {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            origin_x: self.params.origin_x,
            origin_y: self.params.origin_y,
        }
    }
}

impl Default for CircleRevealBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for StairCascade transition
#[derive(Debug, Clone)]
pub struct StairCascadeBuilder {
    params: StairCascadeDefaults,
}

impl StairCascadeBuilder {
    pub fn new() -> Self {
        Self {
            params: StairCascadeDefaults::default(),
        }
    }

    pub fn element_duration_ms(mut self, ms: u64) -> Self {
        self.params.element_duration_ms = ms;
        self
    }

    pub fn stagger_delay_ms(mut self, ms: u64) -> Self {
        self.params.stagger_delay_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn old_page_fade_ms(mut self, ms: u64) -> Self {
        self.params.old_page_fade_ms = ms;
        self
    }

    pub fn translate_distance(mut self, distance: f64) -> Self {
        self.params.translate_distance = distance;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::StairCascade {
            element_duration_ms: self.params.element_duration_ms,
            stagger_delay_ms: self.params.stagger_delay_ms,
            easing: self.params.easing,
            old_page_fade_ms: self.params.old_page_fade_ms,
            translate_distance: self.params.translate_distance,
        }
    }
}

impl Default for StairCascadeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ParallaxSlide transition
#[derive(Debug, Clone)]
pub struct ParallaxSlideBuilder {
    params: ParallaxSlideDefaults,
}

impl ParallaxSlideBuilder {
    pub fn new() -> Self {
        Self {
            params: ParallaxSlideDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.params.duration_ms = ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn old_speed(mut self, speed: f64) -> Self {
        self.params.old_speed = speed;
        self
    }

    pub fn new_speed(mut self, speed: f64) -> Self {
        self.params.new_speed = speed;
        self
    }

    pub fn build(self) -> TransitionAnimation {
        TransitionAnimation::ParallaxSlide {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            old_speed: self.params.old_speed,
            new_speed: self.params.new_speed,
        }
    }
}

impl Default for ParallaxSlideBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_axis_x_builder() {
        let transition = SharedAxisXBuilder::new()
            .enter_duration_ms(250)
            .exit_duration_ms(350)
            .distance(60.0)
            .build();

        if let TransitionAnimation::SharedAxisX {
            enter_duration_ms,
            exit_duration_ms,
            distance,
            ..
        } = transition
        {
            assert_eq!(enter_duration_ms, 250);
            assert_eq!(exit_duration_ms, 350);
            assert_eq!(distance, 60.0);
        } else {
            panic!("Expected SharedAxisX variant");
        }
    }

    #[test]
    fn test_fade_through_builder() {
        let transition = FadeThroughBuilder::new()
            .exit_duration_ms(150)
            .enter_duration_ms(250)
            .enter_scale_from(0.9)
            .build();

        if let TransitionAnimation::FadeThrough {
            exit_duration_ms,
            enter_duration_ms,
            enter_scale_from,
            ..
        } = transition
        {
            assert_eq!(exit_duration_ms, 150);
            assert_eq!(enter_duration_ms, 250);
            assert_eq!(enter_scale_from, 0.9);
        } else {
            panic!("Expected FadeThrough variant");
        }
    }

    #[test]
    fn test_crossfade_builder() {
        let transition = CrossFadeBuilder::new()
            .duration_ms(500)
            .easing(Easing::Linear)
            .build();

        if let TransitionAnimation::CrossFade {
            duration_ms,
            easing,
        } = transition
        {
            assert_eq!(duration_ms, 500);
            assert_eq!(easing, Easing::Linear);
        } else {
            panic!("Expected CrossFade variant");
        }
    }

    #[test]
    fn test_circle_reveal_builder() {
        let transition = CircleRevealBuilder::new()
            .origin(100.0, 200.0)
            .duration_ms(800)
            .build();

        if let TransitionAnimation::CircleReveal {
            origin_x,
            origin_y,
            duration_ms,
            ..
        } = transition
        {
            assert_eq!(origin_x, 100.0);
            assert_eq!(origin_y, 200.0);
            assert_eq!(duration_ms, 800);
        } else {
            panic!("Expected CircleReveal variant");
        }
    }

    #[test]
    fn test_slide_over_builder() {
        let transition = SlideOverBuilder::new()
            .direction(SlideDirection::Left)
            .enter_scale(0.9)
            .build();

        if let TransitionAnimation::SlideOver {
            direction,
            enter_scale,
            ..
        } = transition
        {
            assert_eq!(direction, SlideDirection::Left);
            assert_eq!(enter_scale, 0.9);
        } else {
            panic!("Expected SlideOver variant");
        }
    }

    #[test]
    fn test_stair_cascade_builder() {
        let transition = StairCascadeBuilder::new()
            .element_duration_ms(500)
            .stagger_delay_ms(80)
            .translate_distance(30.0)
            .build();

        if let TransitionAnimation::StairCascade {
            element_duration_ms,
            stagger_delay_ms,
            translate_distance,
            ..
        } = transition
        {
            assert_eq!(element_duration_ms, 500);
            assert_eq!(stagger_delay_ms, 80);
            assert_eq!(translate_distance, 30.0);
        } else {
            panic!("Expected StairCascade variant");
        }
    }

    #[test]
    fn test_builder_defaults() {
        let default_crossfade = CrossFadeBuilder::default().build();
        let manual_crossfade = CrossFadeBuilder::new().build();

        assert_eq!(
            default_crossfade.combined_duration_ms(),
            manual_crossfade.combined_duration_ms()
        );
    }
}
