//! Pre-configured list animation presets
//!
//! Factory functions for common animation patterns based on research from
//! AnimeJS, GSAP, Framer Motion, and production UI libraries.

use super::types::ListAnimation;
use crate::easing::Easing;
use crate::stagger::{GridOrigin, DistanceMetric};
use std::time::Duration;

/// Simple cascade fade-in from bottom (most common list animation)
///
/// Properties animated: opacity (0→1), translateY (30px→0)
/// Timing: 50ms stagger, 300ms per item, ease-out-cubic
pub fn cascade_fade_in() -> ListAnimation {
    ListAnimation::CascadeFadeIn {
        per_item_delay: Duration::from_millis(50),
        item_duration: Duration::from_millis(300),
        easing: Easing::EaseOutCubic,
        slide_distance: 30.0,
    }
}

/// AnimeJS-style center ripple for grids
///
/// Based on: https://codepen.io/juliangarnier/pen/XvjWvx
/// Properties: scale (0.1→1 via 1.2), opacity (0→1)
/// Timing: 100ms per distance unit, 1700ms per item, ease-in-out-quad
pub fn anime_grid_ripple() -> ListAnimation {
    ListAnimation::GridRipple {
        rows: 17,
        cols: 17,
        delay_per_unit: Duration::from_millis(100),
        item_duration: Duration::from_millis(1700),
        easing: Easing::EaseInOutQuad,
        metric: DistanceMetric::Euclidean,
    }
}

/// Grid wave propagating from top-left corner
///
/// Properties: scaleY (0→1), opacity (0→1)
/// Timing: 80ms per column, 500ms per item, power2-in-out
pub fn grid_wave_from_corner() -> ListAnimation {
    ListAnimation::GridWave {
        rows: 5,
        cols: 10,
        origin: GridOrigin::TopLeft,
        delay_per_unit: Duration::from_millis(80),
        item_duration: Duration::from_millis(500),
        easing: Easing::EaseInOutQuad,
        metric: DistanceMetric::Manhattan,
    }
}

/// Diagonal sweep from top-left to bottom-right
///
/// Properties: opacity (0→1), scale (0.8→1), rotation (45°→0°)
/// Timing: 30ms per diagonal step, 600ms per item, ease-in-out
pub fn diagonal_sweep() -> ListAnimation {
    ListAnimation::DiagonalSweep {
        rows: 10,
        cols: 10,
        delay_per_step: Duration::from_millis(30),
        item_duration: Duration::from_millis(600),
        easing: Easing::EaseInOutCubic,
    }
}

/// Masonry grid progressive load
///
/// Based on Masonry library stagger option
/// Properties: opacity (0→1), translateY (20px→0)
/// Timing: 30ms stagger, 400ms per item, ease-out
pub fn masonry_random() -> ListAnimation {
    ListAnimation::MasonryLoad {
        item_duration: Duration::from_millis(400),
        stagger_delay: Duration::from_millis(30),
        easing: Easing::EaseOutCubic,
        slide_distance: 20.0,
    }
}

/// Height expand/collapse animation
///
/// For accordion or list item show/hide
/// Properties: height (0→auto)
/// Timing: 300ms, ease-in-out
pub fn expand_collapse() -> ListAnimation {
    ListAnimation::ExpandCollapse {
        duration: Duration::from_millis(300),
        easing: Easing::EaseInOutCubic,
        from_height: 0.0,
        to_height: 1.0, // Represents auto/full height
    }
}

/// Scale pop-in with overshoot (playful, attention-grabbing)
///
/// Properties: scale (0→1.2→1), opacity (0→1)
/// Timing: 40ms stagger, 600ms per item, back-out easing
pub fn scale_pop_stagger() -> ListAnimation {
    ListAnimation::ScalePopIn {
        per_item_delay: Duration::from_millis(40),
        item_duration: Duration::from_millis(600),
        easing: Easing::EaseOutBack,
        overshoot: 1.2,
    }
}

/// Slide in from left side
///
/// Properties: translateX (-20px→0), opacity (0→1)
/// Timing: 60ms stagger, 400ms per item, ease-out
pub fn slide_from_left() -> ListAnimation {
    ListAnimation::SlideFromSide {
        per_item_delay: Duration::from_millis(60),
        item_duration: Duration::from_millis(400),
        easing: Easing::EaseOutCubic,
        slide_distance: -20.0,
        from_left: true,
    }
}

/// Slide in from right side
///
/// Properties: translateX (20px→0), opacity (0→1)
/// Timing: 60ms stagger, 400ms per item, ease-out
pub fn slide_from_right() -> ListAnimation {
    ListAnimation::SlideFromSide {
        per_item_delay: Duration::from_millis(60),
        item_duration: Duration::from_millis(400),
        easing: Easing::EaseOutCubic,
        slide_distance: 20.0,
        from_left: false,
    }
}

/// Framer Motion stagger children pattern
///
/// Based on: https://www.framer.com/motion/stagger/
/// Parent animates first, then children with stagger
/// Timing: 200ms delay before children, 100ms stagger between, 400ms per item
pub fn framer_stagger_children() -> ListAnimation {
    ListAnimation::FramerStagger {
        delay_children: Duration::from_millis(200),
        stagger_children: Duration::from_millis(100),
        item_duration: Duration::from_millis(400),
        easing: Easing::EaseOutCubic,
    }
}

/// Checkerboard alternating reveal
///
/// Even and odd tiles appear at different times
/// Properties: opacity (0→1), scale (0.8→1)
/// Timing: even tiles 0-800ms, odd tiles 400-1200ms
pub fn checkerboard_reveal() -> ListAnimation {
    ListAnimation::CheckerboardReveal {
        rows: 8,
        cols: 8,
        even_delay: Duration::from_millis(0),
        odd_delay: Duration::from_millis(400),
        item_duration: Duration::from_millis(400),
        easing: Easing::EaseInOutCubic,
    }
}

/// Spiral reveal from center outward
///
/// Based on: https://codepen.io/oemueller/pen/RvOJwG
/// Properties: scale (0→1), rotation (180°→0°), opacity (0→1)
/// Timing: 30ms per step along spiral, 600ms per item, elastic-out
pub fn spiral_reveal() -> ListAnimation {
    ListAnimation::SpiralReveal {
        rows: 8,
        cols: 8,
        delay_per_step: Duration::from_millis(30),
        item_duration: Duration::from_millis(600),
        easing: Easing::EaseOutElastic,
    }
}

/// Snake/zigzag path through grid
///
/// Rows alternate: left→right, right→left
/// Properties: opacity (0→1), scale (0.8→1)
/// Timing: 40ms per cell, 500ms per item, ease-out-quad
pub fn snake_pattern() -> ListAnimation {
    ListAnimation::SnakePattern {
        rows: 10,
        cols: 10,
        delay_per_step: Duration::from_millis(40),
        item_duration: Duration::from_millis(500),
        easing: Easing::EaseOutQuad,
    }
}

/// FLIP reorder animation
///
/// For smooth list reordering using FLIP technique
/// Properties: translateX/Y (delta→0)
/// Timing: 400ms, ease-in-out
pub fn flip_reorder() -> ListAnimation {
    ListAnimation::FlipReorder {
        duration: Duration::from_millis(400),
        easing: Easing::EaseInOutCubic,
    }
}

/// Fast cascade for quick reveals
///
/// Faster version of cascade_fade_in for performance-critical contexts
/// Properties: opacity (0→1), translateY (20px→0)
/// Timing: 30ms stagger, 200ms per item, ease-out
pub fn cascade_fast() -> ListAnimation {
    ListAnimation::CascadeFadeIn {
        per_item_delay: Duration::from_millis(30),
        item_duration: Duration::from_millis(200),
        easing: Easing::EaseOutQuad,
        slide_distance: 20.0,
    }
}

/// Slow, dramatic cascade
///
/// For hero sections or important content reveals
/// Properties: opacity (0→1), translateY (50px→0)
/// Timing: 100ms stagger, 600ms per item, ease-out-expo
pub fn cascade_dramatic() -> ListAnimation {
    ListAnimation::CascadeFadeIn {
        per_item_delay: Duration::from_millis(100),
        item_duration: Duration::from_millis(600),
        easing: Easing::EaseOutExpo,
        slide_distance: 50.0,
    }
}

/// Grid reveal from all four corners (diamond pattern)
///
/// Items closest to any corner appear first
/// Properties: scale (0→1), opacity (0→1), translateZ (-200→0)
/// Timing: 50ms per distance unit, 600ms per item, ease-out-expo
pub fn grid_from_corners() -> ListAnimation {
    // This would need a custom variant, but we can approximate with center + reverse
    ListAnimation::GridWave {
        rows: 10,
        cols: 10,
        origin: GridOrigin::Center,
        delay_per_unit: Duration::from_millis(50),
        item_duration: Duration::from_millis(600),
        easing: Easing::EaseOutExpo,
        metric: DistanceMetric::Euclidean,
    }
}

/// Compact grid stagger for dense layouts
///
/// Minimal stagger delays for space-efficient grids
/// Properties: opacity (0→1), scale (0.9→1)
/// Timing: 20ms per unit, 300ms per item, ease-out
pub fn grid_compact() -> ListAnimation {
    ListAnimation::GridRipple {
        rows: 8,
        cols: 8,
        delay_per_unit: Duration::from_millis(20),
        item_duration: Duration::from_millis(300),
        easing: Easing::EaseOutCubic,
        metric: DistanceMetric::Manhattan,
    }
}

/// Large grid with slow propagation
///
/// For hero grids or full-screen backgrounds
/// Properties: scale (0→1), opacity (0→1)
/// Timing: 150ms per unit, 1000ms per item, ease-in-out-quad
pub fn grid_large() -> ListAnimation {
    ListAnimation::GridRipple {
        rows: 20,
        cols: 20,
        delay_per_unit: Duration::from_millis(150),
        item_duration: Duration::from_millis(1000),
        easing: Easing::EaseInOutQuad,
        metric: DistanceMetric::Euclidean,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_presets_instantiate() {
        // Ensure all presets compile and return valid animations
        let _a1 = cascade_fade_in();
        let _a2 = anime_grid_ripple();
        let _a3 = grid_wave_from_corner();
        let _a4 = diagonal_sweep();
        let _a5 = masonry_random();
        let _a6 = expand_collapse();
        let _a7 = scale_pop_stagger();
        let _a8 = slide_from_left();
        let _a9 = slide_from_right();
        let _a10 = framer_stagger_children();
        let _a11 = checkerboard_reveal();
        let _a12 = spiral_reveal();
        let _a13 = snake_pattern();
        let _a14 = flip_reorder();
        let _a15 = cascade_fast();
        let _a16 = cascade_dramatic();
        let _a17 = grid_from_corners();
        let _a18 = grid_compact();
        let _a19 = grid_large();
    }

    #[test]
    fn test_cascade_generates_delays() {
        let anim = cascade_fade_in();
        let delays = anim.delays_for_count(5);
        assert_eq!(delays.len(), 5);
        assert!(delays[0] < delays[1]);
        assert!(delays[1] < delays[2]);
    }

    #[test]
    fn test_grid_ripple_generates_delays() {
        let anim = anime_grid_ripple();
        let delays = anim.delays_for_grid(17, 17);
        assert_eq!(delays.len(), 17 * 17);
    }

    #[test]
    fn test_framer_stagger_has_delay_children() {
        let anim = framer_stagger_children();
        let delays = anim.delays_for_count(5);
        assert_eq!(delays.len(), 5);
        // All delays should start after delay_children (200ms)
        assert!(delays[0] >= Duration::from_millis(200));
    }
}
