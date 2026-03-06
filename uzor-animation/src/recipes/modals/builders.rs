//! Builder pattern for modal animations
//!
//! Provides fluent builder APIs for customizing modal animations.

use crate::{Easing, Spring, Decay};
use super::{ModalAnimation, defaults::*};

/// Builder for FadeScale animation
#[derive(Debug, Clone)]
pub struct FadeScaleBuilder {
    params: FadeScaleDefaults,
}

impl FadeScaleBuilder {
    pub fn new() -> Self {
        Self {
            params: FadeScaleDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn scale_from(mut self, scale_from: f64) -> Self {
        self.params.scale_from = scale_from;
        self
    }

    pub fn scale_to(mut self, scale_to: f64) -> Self {
        self.params.scale_to = scale_to;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::FadeScale {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            scale_from: self.params.scale_from,
            scale_to: self.params.scale_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
        }
    }
}

impl Default for FadeScaleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SlideUp animation
#[derive(Debug, Clone)]
pub struct SlideUpBuilder {
    params: SlideUpDefaults,
}

impl SlideUpBuilder {
    pub fn new() -> Self {
        Self {
            params: SlideUpDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn translate_from(mut self, translate_from: f64) -> Self {
        self.params.translate_from = translate_from;
        self
    }

    pub fn translate_to(mut self, translate_to: f64) -> Self {
        self.params.translate_to = translate_to;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn delay_ms(mut self, delay_ms: u64) -> Self {
        self.params.delay_ms = delay_ms;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::SlideUp {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            translate_from: self.params.translate_from,
            translate_to: self.params.translate_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
            delay_ms: self.params.delay_ms,
        }
    }
}

impl Default for SlideUpBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SlideDown animation
#[derive(Debug, Clone)]
pub struct SlideDownBuilder {
    params: SlideDownDefaults,
}

impl SlideDownBuilder {
    pub fn new() -> Self {
        Self {
            params: SlideDownDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn translate_from(mut self, translate_from: f64) -> Self {
        self.params.translate_from = translate_from;
        self
    }

    pub fn translate_to(mut self, translate_to: f64) -> Self {
        self.params.translate_to = translate_to;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::SlideDown {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            translate_from: self.params.translate_from,
            translate_to: self.params.translate_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
        }
    }
}

impl Default for SlideDownBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SlideRight animation
#[derive(Debug, Clone)]
pub struct SlideRightBuilder {
    params: SlideRightDefaults,
}

impl SlideRightBuilder {
    pub fn new() -> Self {
        Self {
            params: SlideRightDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn translate_from(mut self, translate_from: f64) -> Self {
        self.params.translate_from = translate_from;
        self
    }

    pub fn translate_to(mut self, translate_to: f64) -> Self {
        self.params.translate_to = translate_to;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::SlideRight {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            translate_from: self.params.translate_from,
            translate_to: self.params.translate_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
        }
    }
}

impl Default for SlideRightBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for SpringScale animation
#[derive(Debug, Clone)]
pub struct SpringScaleBuilder {
    params: SpringScaleDefaults,
}

impl SpringScaleBuilder {
    pub fn new() -> Self {
        Self {
            params: SpringScaleDefaults::default(),
        }
    }

    pub fn spring(mut self, spring: Spring) -> Self {
        self.params.spring = spring;
        self
    }

    pub fn stiffness(mut self, stiffness: f64) -> Self {
        self.params.spring = self.params.spring.stiffness(stiffness);
        self
    }

    pub fn damping(mut self, damping: f64) -> Self {
        self.params.spring = self.params.spring.damping(damping);
        self
    }

    pub fn mass(mut self, mass: f64) -> Self {
        self.params.spring = self.params.spring.mass(mass);
        self
    }

    pub fn scale_from(mut self, scale_from: f64) -> Self {
        self.params.scale_from = scale_from;
        self
    }

    pub fn scale_to(mut self, scale_to: f64) -> Self {
        self.params.scale_to = scale_to;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::SpringScale {
            spring: self.params.spring,
            scale_from: self.params.scale_from,
            scale_to: self.params.scale_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
        }
    }
}

impl Default for SpringScaleBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for Backdrop animation
#[derive(Debug, Clone)]
pub struct BackdropBuilder {
    params: BackdropDefaults,
}

impl BackdropBuilder {
    pub fn new() -> Self {
        Self {
            params: BackdropDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn blur_from(mut self, blur_from: f64) -> Self {
        self.params.blur_from = blur_from;
        self
    }

    pub fn blur_to(mut self, blur_to: f64) -> Self {
        self.params.blur_to = blur_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::Backdrop {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
            blur_from: self.params.blur_from,
            blur_to: self.params.blur_to,
        }
    }
}

impl Default for BackdropBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for DrawerSnap animation
#[derive(Debug, Clone)]
pub struct DrawerSnapBuilder {
    params: DrawerSnapDefaults,
}

impl DrawerSnapBuilder {
    pub fn new() -> Self {
        Self {
            params: DrawerSnapDefaults::default(),
        }
    }

    pub fn spring(mut self, spring: Spring) -> Self {
        self.params.spring = spring;
        self
    }

    pub fn decay(mut self, decay: Option<Decay>) -> Self {
        self.params.decay = decay;
        self
    }

    pub fn snap_points(mut self, snap_points: Vec<f64>) -> Self {
        self.params.snap_points = snap_points;
        self
    }

    pub fn translate_from(mut self, translate_from: f64) -> Self {
        self.params.translate_from = translate_from;
        self
    }

    pub fn translate_to(mut self, translate_to: f64) -> Self {
        self.params.translate_to = translate_to;
        self
    }

    pub fn background_scale_from(mut self, background_scale_from: f64) -> Self {
        self.params.background_scale_from = background_scale_from;
        self
    }

    pub fn background_scale_to(mut self, background_scale_to: f64) -> Self {
        self.params.background_scale_to = background_scale_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::DrawerSnap {
            spring: self.params.spring,
            decay: self.params.decay,
            snap_points: self.params.snap_points,
            translate_from: self.params.translate_from,
            translate_to: self.params.translate_to,
            background_scale_from: self.params.background_scale_from,
            background_scale_to: self.params.background_scale_to,
        }
    }
}

impl Default for DrawerSnapBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for DropIn animation
#[derive(Debug, Clone)]
pub struct DropInBuilder {
    params: DropInDefaults,
}

impl DropInBuilder {
    pub fn new() -> Self {
        Self {
            params: DropInDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn translate_from(mut self, translate_from: f64) -> Self {
        self.params.translate_from = translate_from;
        self
    }

    pub fn translate_to(mut self, translate_to: f64) -> Self {
        self.params.translate_to = translate_to;
        self
    }

    pub fn scale_from(mut self, scale_from: f64) -> Self {
        self.params.scale_from = scale_from;
        self
    }

    pub fn scale_to(mut self, scale_to: f64) -> Self {
        self.params.scale_to = scale_to;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::DropIn {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            translate_from: self.params.translate_from,
            translate_to: self.params.translate_to,
            scale_from: self.params.scale_from,
            scale_to: self.params.scale_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
        }
    }
}

impl Default for DropInBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for ZoomFromOrigin animation
#[derive(Debug, Clone)]
pub struct ZoomFromOriginBuilder {
    params: ZoomFromOriginDefaults,
}

impl ZoomFromOriginBuilder {
    pub fn new() -> Self {
        Self {
            params: ZoomFromOriginDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
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

    pub fn scale_from(mut self, scale_from: f64) -> Self {
        self.params.scale_from = scale_from;
        self
    }

    pub fn scale_to(mut self, scale_to: f64) -> Self {
        self.params.scale_to = scale_to;
        self
    }

    pub fn translate_from(mut self, x: f64, y: f64) -> Self {
        self.params.translate_x_from = x;
        self.params.translate_y_from = y;
        self
    }

    pub fn translate_to(mut self, x: f64, y: f64) -> Self {
        self.params.translate_x_to = x;
        self.params.translate_y_to = y;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::ZoomFromOrigin {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            origin_x: self.params.origin_x,
            origin_y: self.params.origin_y,
            scale_from: self.params.scale_from,
            scale_to: self.params.scale_to,
            translate_x_from: self.params.translate_x_from,
            translate_x_to: self.params.translate_x_to,
            translate_y_from: self.params.translate_y_from,
            translate_y_to: self.params.translate_y_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
        }
    }
}

impl Default for ZoomFromOriginBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for Curtain animation
#[derive(Debug, Clone)]
pub struct CurtainBuilder {
    params: CurtainDefaults,
}

impl CurtainBuilder {
    pub fn new() -> Self {
        Self {
            params: CurtainDefaults::default(),
        }
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.params.duration_ms = duration_ms;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.params.easing = easing;
        self
    }

    pub fn clip_from(mut self, clip_from: f64) -> Self {
        self.params.clip_from = clip_from;
        self
    }

    pub fn clip_to(mut self, clip_to: f64) -> Self {
        self.params.clip_to = clip_to;
        self
    }

    pub fn opacity_from(mut self, opacity_from: f64) -> Self {
        self.params.opacity_from = opacity_from;
        self
    }

    pub fn opacity_to(mut self, opacity_to: f64) -> Self {
        self.params.opacity_to = opacity_to;
        self
    }

    pub fn build(self) -> ModalAnimation {
        ModalAnimation::Curtain {
            duration_ms: self.params.duration_ms,
            easing: self.params.easing,
            clip_from: self.params.clip_from,
            clip_to: self.params.clip_to,
            opacity_from: self.params.opacity_from,
            opacity_to: self.params.opacity_to,
        }
    }
}

impl Default for CurtainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fade_scale_builder() {
        let anim = FadeScaleBuilder::new()
            .duration_ms(300)
            .scale_from(0.9)
            .scale_to(1.0)
            .build();

        assert_eq!(anim.duration_ms(), 300);

        match anim {
            ModalAnimation::FadeScale { scale_from, .. } => {
                assert!((scale_from - 0.9).abs() < 0.01);
            }
            _ => panic!("Expected FadeScale"),
        }
    }

    #[test]
    fn test_spring_scale_builder() {
        let anim = SpringScaleBuilder::new()
            .stiffness(400.0)
            .damping(30.0)
            .scale_from(0.8)
            .build();

        match anim {
            ModalAnimation::SpringScale { spring, scale_from, .. } => {
                assert!((spring.stiffness - 400.0).abs() < 0.1);
                assert!((spring.damping - 30.0).abs() < 0.1);
                assert!((scale_from - 0.8).abs() < 0.01);
            }
            _ => panic!("Expected SpringScale"),
        }
    }

    #[test]
    fn test_slide_up_builder() {
        let anim = SlideUpBuilder::new()
            .duration_ms(500)
            .delay_ms(50)
            .translate_from(120.0)
            .build();

        match anim {
            ModalAnimation::SlideUp {
                duration_ms,
                translate_from,
                delay_ms,
                ..
            } => {
                assert_eq!(duration_ms, 500);
                assert_eq!(delay_ms, 50);
                assert!((translate_from - 120.0).abs() < 0.01);
            }
            _ => panic!("Expected SlideUp"),
        }
    }

    #[test]
    fn test_backdrop_builder() {
        let anim = BackdropBuilder::new()
            .duration_ms(250)
            .blur_to(10.0)
            .build();

        match anim {
            ModalAnimation::Backdrop {
                duration_ms,
                blur_to,
                ..
            } => {
                assert_eq!(duration_ms, 250);
                assert!((blur_to - 10.0).abs() < 0.01);
            }
            _ => panic!("Expected Backdrop"),
        }
    }

    #[test]
    fn test_zoom_from_origin_builder() {
        let anim = ZoomFromOriginBuilder::new()
            .origin(100.0, 200.0)
            .scale_from(0.5)
            .build();

        match anim {
            ModalAnimation::ZoomFromOrigin {
                origin_x,
                origin_y,
                scale_from,
                ..
            } => {
                assert!((origin_x - 100.0).abs() < 0.01);
                assert!((origin_y - 200.0).abs() < 0.01);
                assert!((scale_from - 0.5).abs() < 0.01);
            }
            _ => panic!("Expected ZoomFromOrigin"),
        }
    }

    #[test]
    fn test_drawer_snap_builder() {
        let anim = DrawerSnapBuilder::new()
            .snap_points(vec![0.0, 50.0, 100.0])
            .build();

        match anim {
            ModalAnimation::DrawerSnap { snap_points, .. } => {
                assert_eq!(snap_points.len(), 3);
                assert!((snap_points[1] - 50.0).abs() < 0.01);
            }
            _ => panic!("Expected DrawerSnap"),
        }
    }
}
