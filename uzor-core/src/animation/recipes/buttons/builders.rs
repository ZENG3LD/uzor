//! Builder pattern for customizing button animations
//!
//! Provides fluent API for creating customized animation configurations.

use super::defaults::*;
use super::types::{ButtonAnimation, SlideOrigin, SweepDirection};
use crate::animation::{Easing, Spring};

/// Builder for hover animations
pub struct HoverBuilder {
    duration_ms: u64,
    easing: Easing,
    opacity_from: f64,
    opacity_to: f64,
}

impl HoverBuilder {
    pub fn new() -> Self {
        let defaults = HoverDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            opacity_from: defaults.opacity_from,
            opacity_to: defaults.opacity_to,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn opacity(mut self, from: f64, to: f64) -> Self {
        self.opacity_from = from;
        self.opacity_to = to;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::Hover {
            duration_ms: self.duration_ms,
            easing: self.easing,
            opacity_from: self.opacity_from,
            opacity_to: self.opacity_to,
        }
    }
}

impl Default for HoverBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for press animations
pub struct PressBuilder {
    duration_ms: u64,
    easing: Easing,
    scale: f64,
}

impl PressBuilder {
    pub fn new() -> Self {
        let defaults = PressDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            scale: defaults.scale,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn scale(mut self, s: f64) -> Self {
        self.scale = s;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::Press {
            duration_ms: self.duration_ms,
            easing: self.easing,
            scale: self.scale,
        }
    }
}

impl Default for PressBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for release (spring-back) animations
pub struct ReleaseBuilder {
    spring: Spring,
}

impl ReleaseBuilder {
    pub fn new() -> Self {
        let defaults = ReleaseDefaults::default();
        Self {
            spring: defaults.spring,
        }
    }

    pub fn spring(mut self, s: Spring) -> Self {
        self.spring = s;
        self
    }

    pub fn stiffness(mut self, s: f64) -> Self {
        self.spring = self.spring.stiffness(s);
        self
    }

    pub fn damping(mut self, d: f64) -> Self {
        self.spring = self.spring.damping(d);
        self
    }

    pub fn mass(mut self, m: f64) -> Self {
        self.spring = self.spring.mass(m);
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::Release {
            spring: self.spring,
        }
    }
}

impl Default for ReleaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ripple effect
pub struct RippleBuilder {
    duration_ms: u64,
    easing: Easing,
    scale_from: f64,
    scale_to: f64,
    opacity_from: f64,
    opacity_to: f64,
}

impl RippleBuilder {
    pub fn new() -> Self {
        let defaults = RippleDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            scale_from: defaults.scale_from,
            scale_to: defaults.scale_to,
            opacity_from: defaults.opacity_from,
            opacity_to: defaults.opacity_to,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn scale(mut self, from: f64, to: f64) -> Self {
        self.scale_from = from;
        self.scale_to = to;
        self
    }

    pub fn opacity(mut self, from: f64, to: f64) -> Self {
        self.opacity_from = from;
        self.opacity_to = to;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::Ripple {
            duration_ms: self.duration_ms,
            easing: self.easing,
            scale_from: self.scale_from,
            scale_to: self.scale_to,
            opacity_from: self.opacity_from,
            opacity_to: self.opacity_to,
        }
    }
}

impl Default for RippleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for elastic scale animations
pub struct ElasticScaleBuilder {
    spring: Spring,
    target_scale: f64,
}

impl ElasticScaleBuilder {
    pub fn new() -> Self {
        let defaults = ElasticScaleDefaults::default();
        Self {
            spring: defaults.spring,
            target_scale: defaults.target_scale,
        }
    }

    pub fn spring(mut self, s: Spring) -> Self {
        self.spring = s;
        self
    }

    pub fn stiffness(mut self, s: f64) -> Self {
        self.spring = self.spring.stiffness(s);
        self
    }

    pub fn damping(mut self, d: f64) -> Self {
        self.spring = self.spring.damping(d);
        self
    }

    pub fn target_scale(mut self, s: f64) -> Self {
        self.target_scale = s;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::ElasticScale {
            spring: self.spring,
            target_scale: self.target_scale,
        }
    }
}

impl Default for ElasticScaleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for glow pulse animations
pub struct GlowPulseBuilder {
    duration_ms: u64,
    easing: Easing,
    intensity_from: f64,
    intensity_to: f64,
}

impl GlowPulseBuilder {
    pub fn new() -> Self {
        let defaults = GlowPulseDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            intensity_from: defaults.intensity_from,
            intensity_to: defaults.intensity_to,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn intensity(mut self, from: f64, to: f64) -> Self {
        self.intensity_from = from;
        self.intensity_to = to;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::GlowPulse {
            duration_ms: self.duration_ms,
            easing: self.easing,
            intensity_from: self.intensity_from,
            intensity_to: self.intensity_to,
        }
    }
}

impl Default for GlowPulseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for underline slide animations
pub struct UnderlineSlideBuilder {
    duration_ms: u64,
    easing: Easing,
    origin: SlideOrigin,
}

impl UnderlineSlideBuilder {
    pub fn new() -> Self {
        let defaults = UnderlineSlideDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            origin: SlideOrigin::Left,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn origin(mut self, o: SlideOrigin) -> Self {
        self.origin = o;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::UnderlineSlide {
            duration_ms: self.duration_ms,
            easing: self.easing,
            origin: self.origin,
        }
    }
}

impl Default for UnderlineSlideBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for fill sweep animations
pub struct FillSweepBuilder {
    duration_ms: u64,
    easing: Easing,
    direction: SweepDirection,
}

impl FillSweepBuilder {
    pub fn new() -> Self {
        let defaults = FillSweepDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            direction: SweepDirection::LeftToRight,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn direction(mut self, d: SweepDirection) -> Self {
        self.direction = d;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::FillSweep {
            duration_ms: self.duration_ms,
            easing: self.easing,
            direction: self.direction,
        }
    }
}

impl Default for FillSweepBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for border draw animations
pub struct BorderDrawBuilder {
    duration_ms: u64,
    easing: Easing,
    stagger_delay_ms: u64,
}

impl BorderDrawBuilder {
    pub fn new() -> Self {
        let defaults = BorderDrawDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            stagger_delay_ms: defaults.stagger_delay_ms,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn stagger_delay_ms(mut self, ms: u64) -> Self {
        self.stagger_delay_ms = ms;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::BorderDraw {
            duration_ms: self.duration_ms,
            easing: self.easing,
            stagger_delay_ms: self.stagger_delay_ms,
        }
    }
}

impl Default for BorderDrawBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for magnetic pull animations
pub struct MagneticPullBuilder {
    spring: Spring,
    max_distance: f64,
    strength: f64,
}

impl MagneticPullBuilder {
    pub fn new() -> Self {
        let defaults = MagneticPullDefaults::default();
        Self {
            spring: defaults.spring,
            max_distance: defaults.max_distance,
            strength: defaults.strength,
        }
    }

    pub fn spring(mut self, s: Spring) -> Self {
        self.spring = s;
        self
    }

    pub fn stiffness(mut self, s: f64) -> Self {
        self.spring = self.spring.stiffness(s);
        self
    }

    pub fn damping(mut self, d: f64) -> Self {
        self.spring = self.spring.damping(d);
        self
    }

    pub fn max_distance(mut self, d: f64) -> Self {
        self.max_distance = d;
        self
    }

    pub fn strength(mut self, s: f64) -> Self {
        self.strength = s;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::MagneticPull {
            spring: self.spring,
            max_distance: self.max_distance,
            strength: self.strength,
        }
    }
}

impl Default for MagneticPullBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for lift shadow animations
pub struct LiftShadowBuilder {
    duration_ms: u64,
    easing: Easing,
    shadow_y_from: f64,
    shadow_y_to: f64,
    shadow_blur_from: f64,
    shadow_blur_to: f64,
    lift_distance: f64,
}

impl LiftShadowBuilder {
    pub fn new() -> Self {
        let defaults = LiftShadowDefaults::default();
        Self {
            duration_ms: defaults.duration_ms,
            easing: defaults.easing,
            shadow_y_from: defaults.shadow_y_from,
            shadow_y_to: defaults.shadow_y_to,
            shadow_blur_from: defaults.shadow_blur_from,
            shadow_blur_to: defaults.shadow_blur_to,
            lift_distance: defaults.lift_distance,
        }
    }

    pub fn duration_ms(mut self, ms: u64) -> Self {
        self.duration_ms = ms;
        self
    }

    pub fn easing(mut self, e: Easing) -> Self {
        self.easing = e;
        self
    }

    pub fn shadow_y(mut self, from: f64, to: f64) -> Self {
        self.shadow_y_from = from;
        self.shadow_y_to = to;
        self
    }

    pub fn shadow_blur(mut self, from: f64, to: f64) -> Self {
        self.shadow_blur_from = from;
        self.shadow_blur_to = to;
        self
    }

    pub fn lift_distance(mut self, d: f64) -> Self {
        self.lift_distance = d;
        self
    }

    pub fn build(self) -> ButtonAnimation {
        ButtonAnimation::LiftShadow {
            duration_ms: self.duration_ms,
            easing: self.easing,
            shadow_y_from: self.shadow_y_from,
            shadow_y_to: self.shadow_y_to,
            shadow_blur_from: self.shadow_blur_from,
            shadow_blur_to: self.shadow_blur_to,
            lift_distance: self.lift_distance,
        }
    }
}

impl Default for LiftShadowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hover_builder() {
        let anim = HoverBuilder::new()
            .duration_ms(300)
            .opacity(0.0, 0.1)
            .easing(Easing::EaseInQuad)
            .build();

        assert_eq!(anim.duration_ms(), 300);
    }

    #[test]
    fn test_press_builder() {
        let anim = PressBuilder::new().scale(0.9).duration_ms(150).build();

        assert_eq!(anim.duration_ms(), 150);
    }

    #[test]
    fn test_release_builder() {
        let anim = ReleaseBuilder::new().stiffness(200.0).damping(15.0).build();

        let duration = anim.duration_ms();
        assert!(duration > 0);
    }

    #[test]
    fn test_ripple_builder() {
        let anim = RippleBuilder::new()
            .duration_ms(500)
            .scale(0.0, 5.0)
            .opacity(0.15, 0.0)
            .build();

        assert_eq!(anim.duration_ms(), 500);
    }

    #[test]
    fn test_elastic_scale_builder() {
        let anim = ElasticScaleBuilder::new()
            .stiffness(250.0)
            .target_scale(1.15)
            .build();

        let duration = anim.duration_ms();
        assert!(duration > 0);
    }

    #[test]
    fn test_all_builders() {
        // Verify all builders work
        let _ = HoverBuilder::new().build();
        let _ = PressBuilder::new().build();
        let _ = ReleaseBuilder::new().build();
        let _ = RippleBuilder::new().build();
        let _ = ElasticScaleBuilder::new().build();
        let _ = GlowPulseBuilder::new().build();
        let _ = UnderlineSlideBuilder::new().build();
        let _ = FillSweepBuilder::new().build();
        let _ = BorderDrawBuilder::new().build();
        let _ = MagneticPullBuilder::new().build();
        let _ = LiftShadowBuilder::new().build();
    }
}
