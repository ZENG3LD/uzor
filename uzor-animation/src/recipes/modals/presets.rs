//! Modal animation presets
//!
//! Factory functions for common modal animation patterns from Material Design,
//! iOS, Framer Motion, Radix UI, and other UI libraries.

use crate::{Easing, Spring, Decay};
use super::ModalAnimation;

/// Material Design dialog (225ms fade + scale)
///
/// Fade in from 0 to 1 while scaling from 0.95 to 1.0.
/// Uses Material deceleration curve (ease-out).
///
/// Source: Material Design 1 - Duration & Easing
pub fn material_dialog() -> ModalAnimation {
    ModalAnimation::FadeScale {
        duration_ms: 225,
        easing: Easing::CubicBezier(0.0, 0.0, 0.2, 1.0), // Material decelerate
        scale_from: 0.95,
        scale_to: 1.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// Material Design dialog exit (195ms fade + scale)
///
/// Slightly faster than entrance with acceleration curve.
///
/// Source: Material Design 1 - Duration & Easing
pub fn material_dialog_exit() -> ModalAnimation {
    ModalAnimation::FadeScale {
        duration_ms: 195,
        easing: Easing::CubicBezier(0.4, 0.0, 1.0, 1.0), // Material accelerate
        scale_from: 1.0,
        scale_to: 0.95,
        opacity_from: 1.0,
        opacity_to: 0.0,
    }
}

/// Material Design bottom sheet (450ms slide up)
///
/// Sheet slides up from bottom with 100ms delay, overlay fades in.
/// Uses Material emphasized curve for smooth motion.
///
/// Source: Material Design Bottom Sheet CodePen
pub fn material_bottom_sheet() -> ModalAnimation {
    ModalAnimation::SlideUp {
        duration_ms: 450,
        easing: Easing::CubicBezier(0.32, 1.0, 0.23, 1.0), // Material emphasized
        translate_from: 100.0,
        translate_to: 0.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
        delay_ms: 100,
    }
}

/// iOS alert dialog (spring-based)
///
/// Alert dialog with spring physics. Stiffness 350, damping 28 for
/// iOS-native feel with subtle bounce.
///
/// Source: iOS HIG + Framer Motion spring configs
pub fn ios_alert() -> ModalAnimation {
    let spring = Spring::new()
        .stiffness(350.0)
        .damping(28.0)
        .mass(1.0);

    ModalAnimation::SpringScale {
        spring,
        scale_from: 0.0,
        scale_to: 1.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// iOS bottom sheet (500ms spring slide up)
///
/// iOS-style sheet presentation with spring timing curve.
/// Smoother and more elastic than Material.
///
/// Source: Building a Drawer Component (emilkowal.ski)
pub fn ios_sheet() -> ModalAnimation {
    ModalAnimation::SlideUp {
        duration_ms: 500,
        easing: Easing::CubicBezier(0.32, 0.72, 0.0, 1.0), // iOS curve from Ionic
        translate_from: 100.0,
        translate_to: 0.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
        delay_ms: 0,
    }
}

/// Vaul drawer with snap points and momentum
///
/// Bottom drawer with spring physics, snap points, and decay momentum.
/// Background scales to 0.95 when drawer is open (40% drag).
///
/// Source: Vaul library (emilkowalski/vaul)
pub fn vaul_drawer() -> ModalAnimation {
    let spring = Spring::new()
        .stiffness(300.0)
        .damping(30.0)
        .mass(1.0);

    let decay = Some(Decay::new(0.0).friction(0.998));

    ModalAnimation::DrawerSnap {
        spring,
        decay,
        snap_points: vec![0.0, 40.0, 100.0], // Closed, half, full
        translate_from: 100.0,
        translate_to: 0.0,
        background_scale_from: 1.0,
        background_scale_to: 0.95,
    }
}

/// Fade backdrop overlay (200ms linear)
///
/// Simple backdrop fade from transparent to semi-transparent.
/// Uses linear easing for steady, predictable fade.
///
/// Source: Common pattern across UI libraries
pub fn fade_backdrop() -> ModalAnimation {
    ModalAnimation::Backdrop {
        duration_ms: 200,
        easing: Easing::Linear,
        opacity_from: 0.0,
        opacity_to: 0.5,
        blur_from: 0.0,
        blur_to: 0.0,
    }
}

/// Blur backdrop overlay (300ms ease-out)
///
/// Backdrop with blur effect (frosted glass).
/// Fades to 0.4 opacity and blurs to 8px.
///
/// Source: Backdrop Blur Modal CSS Guide
pub fn blur_backdrop() -> ModalAnimation {
    ModalAnimation::Backdrop {
        duration_ms: 300,
        easing: Easing::EASE_OUT,
        opacity_from: 0.0,
        opacity_to: 0.4,
        blur_from: 0.0,
        blur_to: 8.0,
    }
}

/// Side panel slide from right (300ms ease-out-cubic)
///
/// Panel slides in from right edge. Common for navigation drawers
/// and settings panels.
///
/// Source: Slide-in Drawer with React & Tailwind
pub fn slide_panel_right() -> ModalAnimation {
    ModalAnimation::SlideRight {
        duration_ms: 300,
        easing: Easing::EaseOutCubic,
        translate_from: 100.0,
        translate_to: 0.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// Drop bounce animation (500ms with bounce easing)
///
/// Modal drops from above with bounce effect. Scales from 0.1 to 1.0
/// while dropping. Creates playful, attention-grabbing entrance.
///
/// Source: CSS Modal Bounce (paulrhayes.com)
pub fn drop_bounce() -> ModalAnimation {
    ModalAnimation::DropIn {
        duration_ms: 500,
        easing: Easing::EaseOutBounce,
        translate_from: -100.0,
        translate_to: 0.0,
        scale_from: 0.1,
        scale_to: 1.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// Zoom from origin (lightbox style, 300ms)
///
/// Image/content zooms from thumbnail position to fullscreen.
/// Transform origin set to thumbnail center.
///
/// Source: Animating Zooming Using CSS (jakearchibald.com)
pub fn zoom_from_origin(origin_x: f64, origin_y: f64, scale_factor: f64) -> ModalAnimation {
    let translate_x = origin_x * scale_factor;
    let translate_y = origin_y * scale_factor;

    ModalAnimation::ZoomFromOrigin {
        duration_ms: 300,
        easing: Easing::CubicBezier(0.4, 0.0, 0.2, 1.0), // Material standard
        origin_x,
        origin_y,
        scale_from: scale_factor,
        scale_to: 1.0,
        translate_x_from: translate_x,
        translate_x_to: 0.0,
        translate_y_from: translate_y,
        translate_y_to: 0.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// Curtain reveal (clip-path from center, 400ms)
///
/// Clip-path reveals content from center outward. Creates
/// cinematic reveal effect.
///
/// Source: Various creative CSS animation examples
pub fn curtain_reveal() -> ModalAnimation {
    ModalAnimation::Curtain {
        duration_ms: 400,
        easing: Easing::EASE_IN_OUT,
        clip_from: 0.0,
        clip_to: 1.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// Framer Motion spring modal (stiffness 300, damping 25)
///
/// Modal with Framer Motion's recommended spring config.
/// Balanced between snappy and smooth.
///
/// Source: Framer Motion Spring Config Examples
pub fn framer_spring_modal() -> ModalAnimation {
    let spring = Spring::new()
        .stiffness(300.0)
        .damping(25.0)
        .mass(1.0);

    ModalAnimation::SpringScale {
        spring,
        scale_from: 0.9,
        scale_to: 1.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// Radix UI simple fade (300ms ease-out)
///
/// Simple opacity fade used by Radix UI components.
/// Minimal, accessible, works everywhere.
///
/// Source: Radix UI Animation Guide
pub fn radix_fade() -> ModalAnimation {
    ModalAnimation::FadeScale {
        duration_ms: 300,
        easing: Easing::EASE_OUT,
        scale_from: 1.0,
        scale_to: 1.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

/// Radix UI fade exit (300ms ease-in)
///
/// Exit animation for Radix UI components.
pub fn radix_fade_exit() -> ModalAnimation {
    ModalAnimation::FadeScale {
        duration_ms: 300,
        easing: Easing::EASE_IN,
        scale_from: 1.0,
        scale_to: 1.0,
        opacity_from: 1.0,
        opacity_to: 0.0,
    }
}

/// iOS slide down alert (400ms)
///
/// Alert slides down from top with iOS-style curve and fade.
///
/// Source: iOS Modal Animations Overview
pub fn ios_slide_down() -> ModalAnimation {
    ModalAnimation::SlideDown {
        duration_ms: 400,
        easing: Easing::CubicBezier(0.32, 0.72, 0.0, 1.0), // iOS curve
        translate_from: -100.0,
        translate_to: 0.0,
        opacity_from: 0.0,
        opacity_to: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_dialog_params() {
        let anim = material_dialog();
        assert_eq!(anim.duration_ms(), 225);

        match anim {
            ModalAnimation::FadeScale {
                scale_from,
                scale_to,
                opacity_from,
                opacity_to,
                ..
            } => {
                assert!((scale_from - 0.95).abs() < 0.01);
                assert!((scale_to - 1.0).abs() < 0.01);
                assert!((opacity_from - 0.0).abs() < 0.01);
                assert!((opacity_to - 1.0).abs() < 0.01);
            }
            _ => panic!("Expected FadeScale"),
        }
    }

    #[test]
    fn test_ios_alert_spring() {
        let anim = ios_alert();
        assert!(anim.duration_ms() > 0);

        match anim {
            ModalAnimation::SpringScale { spring, .. } => {
                assert!((spring.stiffness - 350.0).abs() < 0.1);
                assert!((spring.damping - 28.0).abs() < 0.1);
            }
            _ => panic!("Expected SpringScale"),
        }
    }

    #[test]
    fn test_vaul_drawer_snap_points() {
        let anim = vaul_drawer();

        match anim {
            ModalAnimation::DrawerSnap { snap_points, .. } => {
                assert_eq!(snap_points.len(), 3);
                assert!((snap_points[0] - 0.0).abs() < 0.01);
                assert!((snap_points[1] - 40.0).abs() < 0.01);
                assert!((snap_points[2] - 100.0).abs() < 0.01);
            }
            _ => panic!("Expected DrawerSnap"),
        }
    }

    #[test]
    fn test_blur_backdrop() {
        let anim = blur_backdrop();

        match anim {
            ModalAnimation::Backdrop {
                blur_from, blur_to, ..
            } => {
                assert!((blur_from - 0.0).abs() < 0.01);
                assert!((blur_to - 8.0).abs() < 0.01);
            }
            _ => panic!("Expected Backdrop"),
        }
    }

    #[test]
    fn test_zoom_from_origin() {
        let anim = zoom_from_origin(50.0, 100.0, 0.2);

        match anim {
            ModalAnimation::ZoomFromOrigin {
                scale_from,
                scale_to,
                translate_x_from,
                translate_y_from,
                ..
            } => {
                assert!((scale_from - 0.2).abs() < 0.01);
                assert!((scale_to - 1.0).abs() < 0.01);
                assert!((translate_x_from - 10.0).abs() < 0.01); // 50 * 0.2
                assert!((translate_y_from - 20.0).abs() < 0.01); // 100 * 0.2
            }
            _ => panic!("Expected ZoomFromOrigin"),
        }
    }

    #[test]
    fn test_all_presets_compile() {
        // Just verify all presets can be constructed
        let _ = material_dialog();
        let _ = material_dialog_exit();
        let _ = material_bottom_sheet();
        let _ = ios_alert();
        let _ = ios_sheet();
        let _ = vaul_drawer();
        let _ = fade_backdrop();
        let _ = blur_backdrop();
        let _ = slide_panel_right();
        let _ = drop_bounce();
        let _ = zoom_from_origin(0.0, 0.0, 0.5);
        let _ = curtain_reveal();
        let _ = framer_spring_modal();
        let _ = radix_fade();
        let _ = radix_fade_exit();
        let _ = ios_slide_down();
    }
}
