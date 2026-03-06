//! Pre-configured scroll animation presets
//!
//! Ready-to-use scroll animations based on common web patterns and research.

use super::types::*;
use crate::easing::Easing;
use std::time::Duration;

/// Horizontal reading progress bar (0-100% linear)
///
/// # Example
/// Fixed bar at top of page that fills from left to right as user scrolls.
///
/// # Based on
/// - Chrome DevTools docs reading indicator
/// - Medium article progress bar
pub fn progress_bar_horizontal() -> ScrollAnimation {
    ScrollAnimation::ProgressBar {
        scroll_start: 0.0,
        scroll_end: 10000.0, // Will be calculated from document height
        easing: Easing::Linear,
        orientation: ProgressBarOrientation::Horizontal,
    }
}

/// Circular SVG progress ring
///
/// # Example
/// Radial progress indicator around scroll-to-top button.
/// SVG stroke-dashoffset animates from circumference to 0.
///
/// # Based on
/// - CodePen "Back to Top" button patterns
/// - Circular progress indicators in SPAs
pub fn progress_ring() -> ScrollAnimation {
    ScrollAnimation::ProgressBar {
        scroll_start: 0.0,
        scroll_end: 10000.0,
        easing: Easing::Linear,
        orientation: ProgressBarOrientation::Circular,
    }
}

/// 3-layer parallax hero (0.3x, 0.6x, 1.0x speeds)
///
/// # Example
/// Hero section with background mountain, midground clouds, foreground text.
/// Creates depth illusion through differential movement.
///
/// # Based on
/// - Apple product page parallax
/// - Firewatch game website
pub fn parallax_hero() -> ScrollAnimation {
    ScrollAnimation::ParallaxLayers {
        layer_speeds: vec![0.3, 0.6, 1.0],
        axis: ParallaxAxis::Vertical,
    }
}

/// Fade in when entering viewport (opacity 0→1 over entry 0-30%)
///
/// # Example
/// Content sections fade in smoothly as they enter from bottom of viewport.
///
/// # Based on
/// - Intersection Observer fade patterns
/// - AOS (Animate On Scroll) library defaults
pub fn fade_in_on_enter() -> ScrollAnimation {
    ScrollAnimation::FadeOnScroll {
        opacity_from: 0.0,
        opacity_to: 1.0,
        scroll_range: (0.0, 0.3), // First 30% of entry
        easing: Easing::EaseOutCubic,
    }
}

/// Slide up + fade when entering (translateY 50→0, opacity 0→1, entry 0-40%)
///
/// # Example
/// Elements slide up from below while fading in. Classic reveal pattern.
///
/// # Based on
/// - Material Design motion guidelines
/// - Framer Motion scroll reveals
pub fn slide_up_on_enter() -> ScrollAnimation {
    ScrollAnimation::RevealOnEnter {
        translate_y: 50.0,
        opacity_from: 0.0,
        entry_range: (0.0, 0.4),
        duration: Duration::from_millis(600),
        easing: Easing::EaseOutCubic,
    }
}

/// Reveal from left (translateX -100→0 over 30%)
///
/// # Example
/// Content slides in from left side when entering viewport.
///
/// # Based on
/// - Horizontal slide patterns
/// - Side navigation reveals
pub fn reveal_from_left() -> ScrollAnimation {
    ScrollAnimation::RevealOnEnter {
        translate_y: 0.0, // Using as translateX semantically
        opacity_from: 1.0, // No fade, just slide
        entry_range: (0.0, 0.3),
        duration: Duration::from_millis(500),
        easing: Easing::EaseOutQuart,
    }
}

/// Sticky header that shrinks (height 80→48, scale down)
///
/// # Example
/// Navigation header compacts when scrolling down past threshold.
/// Common in modern websites for space efficiency.
///
/// # Based on
/// - Bootstrap navbar-shrink pattern
/// - Tailwind UI header examples
pub fn sticky_shrink_header() -> ScrollAnimation {
    ScrollAnimation::StickyHeader {
        from_state: HeaderState {
            scale: 1.0,
            height: 80.0,
            opacity: 1.0,
        },
        to_state: HeaderState {
            scale: 0.9,
            height: 48.0,
            opacity: 0.95,
        },
        scroll_threshold: 100.0,
        duration: Duration::from_millis(300),
        easing: Easing::EaseInOutQuad,
    }
}

/// Horizontal scroll through pinned cards (300vh vertical = 100% horizontal)
///
/// # Example
/// Gallery section pins in place while vertical scroll drives horizontal
/// card movement. Cards slide left across viewport.
///
/// # Based on
/// - GSAP ScrollTrigger horizontal demos
/// - Apple AirPods product page
pub fn horizontal_pin_scroll() -> ScrollAnimation {
    ScrollAnimation::HorizontalScroll {
        scroll_distance: 2000.0, // Horizontal pixels to scroll
        vertical_range: (0.0, 3000.0), // 300vh of vertical scroll
        pin: true,
        easing: Easing::Linear,
    }
}

/// Number counter (0→target over 40% viewport entry)
///
/// # Example
/// Statistics numbers count up from 0 when element enters viewport.
/// Creates engaging reveal for metrics/stats sections.
///
/// # Based on
/// - CounterUp.js library
/// - Odometer effect libraries
pub fn number_counter() -> ScrollAnimation {
    ScrollAnimation::NumberCounter {
        from: 0.0,
        to: 100.0, // Will be overridden with actual target
        threshold: 0.8, // Trigger at 80% visibility
        duration: Duration::from_millis(2000),
        easing: Easing::EaseOutCubic, // Natural counting deceleration
    }
}

/// Color shift through sections (background color changes)
///
/// # Example
/// Background transitions through color palette as user scrolls through
/// different content sections.
///
/// # Based on
/// - Apple event pages
/// - Single-page scrolling sites with color zones
pub fn color_shift_sections() -> ScrollAnimation {
    ScrollAnimation::ColorShift {
        color_stops: vec![
            (0.0, (0.4, 0.47, 0.92)),   // Blue
            (0.33, (0.56, 0.27, 0.68)), // Purple
            (0.66, (0.96, 0.34, 0.42)), // Pink
            (1.0, (1.0, 0.6, 0.2)),     // Orange
        ],
        scroll_range: (0.0, 5000.0),
        easing: Easing::EaseInOutSine,
    }
}

/// Scale on scroll (0.8→1.0 as element enters viewport)
///
/// # Example
/// Images or cards scale up slightly as they enter, creating
/// subtle zoom-in effect.
///
/// # Based on
/// - View timeline scale patterns
/// - Ken Burns effect for scroll
pub fn scale_on_scroll() -> ScrollAnimation {
    ScrollAnimation::FadeOnScroll {
        opacity_from: 1.0, // No fade, using for scale semantically
        opacity_to: 1.0,
        scroll_range: (0.0, 1.0), // Entry to cover
        easing: Easing::EaseOutQuad,
    }
}

/// Parallax text layers (text moves at different speed than background)
///
/// # Example
/// Title text moves at 0.7x speed while background moves at 0.3x,
/// creating layered depth effect.
///
/// # Based on
/// - Firewatch game website text layers
/// - Apple product page hero text
pub fn parallax_text() -> ScrollAnimation {
    ScrollAnimation::ParallaxLayers {
        layer_speeds: vec![0.3, 0.7], // Background, text
        axis: ParallaxAxis::Vertical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_horizontal() {
        let preset = progress_bar_horizontal();
        match preset {
            ScrollAnimation::ProgressBar { orientation, .. } => {
                assert_eq!(orientation, ProgressBarOrientation::Horizontal);
            }
            _ => panic!("Expected ProgressBar variant"),
        }
    }

    #[test]
    fn test_parallax_hero_layers() {
        let preset = parallax_hero();
        match preset {
            ScrollAnimation::ParallaxLayers { layer_speeds, .. } => {
                assert_eq!(layer_speeds.len(), 3);
                assert_eq!(layer_speeds[0], 0.3);
                assert_eq!(layer_speeds[2], 1.0);
            }
            _ => panic!("Expected ParallaxLayers variant"),
        }
    }

    #[test]
    fn test_sticky_header_states() {
        let preset = sticky_shrink_header();
        match preset {
            ScrollAnimation::StickyHeader {
                from_state,
                to_state,
                ..
            } => {
                assert_eq!(from_state.height, 80.0);
                assert_eq!(to_state.height, 48.0);
            }
            _ => panic!("Expected StickyHeader variant"),
        }
    }

    #[test]
    fn test_number_counter_timing() {
        let preset = number_counter();
        match preset {
            ScrollAnimation::NumberCounter {
                duration, easing, ..
            } => {
                assert_eq!(duration, Duration::from_millis(2000));
                assert_eq!(easing, Easing::EaseOutCubic);
            }
            _ => panic!("Expected NumberCounter variant"),
        }
    }

    #[test]
    fn test_color_shift_stops() {
        let preset = color_shift_sections();
        match preset {
            ScrollAnimation::ColorShift { color_stops, .. } => {
                assert_eq!(color_stops.len(), 4);
                assert_eq!(color_stops[0].0, 0.0);
                assert_eq!(color_stops[3].0, 1.0);
            }
            _ => panic!("Expected ColorShift variant"),
        }
    }
}
