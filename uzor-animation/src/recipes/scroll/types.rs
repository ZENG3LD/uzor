//! Scroll animation types catalog
//!
//! Defines all scroll-driven and parallax animation variants with their parameters.
//! Based on research from GSAP ScrollTrigger, CSS Scroll-Driven Animations spec,
//! Locomotive Scroll, and Apple product pages.

use crate::easing::Easing;
use std::time::Duration;

/// Catalog of scroll-driven animation patterns
#[derive(Debug, Clone)]
pub enum ScrollAnimation {
    /// Horizontal/vertical progress bar tracking scroll position (0-100%)
    ///
    /// # Example: Reading progress indicator
    /// Fixed bar at top that fills as user scrolls through document.
    ProgressBar {
        /// Start scroll position (e.g., 0.0 = top of document)
        scroll_start: f64,
        /// End scroll position (e.g., document height)
        scroll_end: f64,
        /// Easing function for progress mapping
        easing: Easing,
        /// Orientation of the progress bar
        orientation: ProgressBarOrientation,
    },

    /// Multi-layer parallax with different scroll speeds
    ///
    /// # Example: Hero background effect
    /// Background moves at 0.3x, midground 0.6x, foreground 1.0x speed.
    ParallaxLayers {
        /// Depth factors for each layer (0.0 = static, 1.0 = full speed)
        layer_speeds: Vec<f64>,
        /// Scroll axis (vertical or horizontal)
        axis: ParallaxAxis,
    },

    /// Element fades in/out based on scroll position
    ///
    /// # Example: Section fade transitions
    /// Fade in when entering viewport, fade out when exiting.
    FadeOnScroll {
        /// Start opacity value
        opacity_from: f64,
        /// End opacity value
        opacity_to: f64,
        /// Scroll range for animation (start, end)
        scroll_range: (f64, f64),
        /// Easing function
        easing: Easing,
    },

    /// Element animates when entering viewport (slide + fade)
    ///
    /// # Example: Content reveal on scroll
    /// Elements slide up and fade in when they enter viewport.
    RevealOnEnter {
        /// Translation distance (positive = from below, negative = from above)
        translate_y: f64,
        /// Start opacity
        opacity_from: f64,
        /// Entry progress range (0.0 = starts entering, 1.0 = fully entered)
        entry_range: (f64, f64),
        /// Duration of the reveal animation
        duration: Duration,
        /// Easing function
        easing: Easing,
    },

    /// Header shrinks/transforms on scroll
    ///
    /// # Example: Sticky nav that compacts
    /// Header height reduces from 80px to 48px when scrolling down.
    StickyHeader {
        /// Initial state values (scale, height, etc.)
        from_state: HeaderState,
        /// Final state values
        to_state: HeaderState,
        /// Scroll threshold to trigger transformation
        scroll_threshold: f64,
        /// Duration of transformation
        duration: Duration,
        /// Easing function
        easing: Easing,
    },

    /// Horizontal scroll within pinned section
    ///
    /// # Example: Gallery cards sliding horizontally
    /// Vertical scroll drives horizontal card movement while section is pinned.
    HorizontalScroll {
        /// Total horizontal distance to scroll
        scroll_distance: f64,
        /// Vertical scroll range that drives the horizontal movement
        vertical_range: (f64, f64),
        /// Should the section be pinned during scroll?
        pin: bool,
        /// Easing function
        easing: Easing,
    },

    /// Number counts up as user scrolls to it
    ///
    /// # Example: Statistics counter
    /// Number animates from 0 to target value when element enters viewport.
    NumberCounter {
        /// Starting value
        from: f64,
        /// Target value
        to: f64,
        /// Viewport visibility threshold to trigger (0.0-1.0)
        threshold: f64,
        /// Duration of counting animation
        duration: Duration,
        /// Easing function (cubic-out recommended for natural counting)
        easing: Easing,
    },

    /// Background color transitions based on scroll position
    ///
    /// # Example: Section color changes
    /// Background shifts through color palette as user scrolls.
    ColorShift {
        /// Color stops (progress, color) - progress in 0.0-1.0
        color_stops: Vec<(f64, (f64, f64, f64))>, // (progress, (r, g, b))
        /// Scroll range for full color transition
        scroll_range: (f64, f64),
        /// Easing function
        easing: Easing,
    },
}

/// Orientation of progress bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBarOrientation {
    /// Horizontal (left to right)
    Horizontal,
    /// Vertical (top to bottom)
    Vertical,
    /// Circular/radial (SVG stroke-based)
    Circular,
}

/// Parallax scroll axis
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallaxAxis {
    /// Vertical scrolling
    Vertical,
    /// Horizontal scrolling
    Horizontal,
}

/// Header state for sticky transformations
#[derive(Debug, Clone, Copy)]
pub struct HeaderState {
    /// Scale factor (1.0 = normal)
    pub scale: f64,
    /// Height in pixels
    pub height: f64,
    /// Opacity (0.0-1.0)
    pub opacity: f64,
}

impl ScrollAnimation {
    /// Get the scroll range that this animation covers
    ///
    /// Returns (start_position, end_position) in scroll coordinates.
    pub fn scroll_range(&self) -> (f64, f64) {
        match self {
            ScrollAnimation::ProgressBar {
                scroll_start,
                scroll_end,
                ..
            } => (*scroll_start, *scroll_end),
            ScrollAnimation::ParallaxLayers { .. } => (0.0, f64::MAX), // Continuous
            ScrollAnimation::FadeOnScroll { scroll_range, .. } => *scroll_range,
            ScrollAnimation::RevealOnEnter { entry_range, .. } => *entry_range,
            ScrollAnimation::StickyHeader {
                scroll_threshold, ..
            } => (*scroll_threshold, *scroll_threshold + 200.0),
            ScrollAnimation::HorizontalScroll {
                vertical_range, ..
            } => *vertical_range,
            ScrollAnimation::NumberCounter { .. } => (0.0, 1000.0), // Viewport-based
            ScrollAnimation::ColorShift { scroll_range, .. } => *scroll_range,
        }
    }

    /// Check if this animation is active at the given scroll position
    pub fn is_active(&self, scroll_position: f64) -> bool {
        let (start, end) = self.scroll_range();
        scroll_position >= start && scroll_position <= end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_scroll_range() {
        let anim = ScrollAnimation::ProgressBar {
            scroll_start: 0.0,
            scroll_end: 1000.0,
            easing: Easing::Linear,
            orientation: ProgressBarOrientation::Horizontal,
        };

        assert_eq!(anim.scroll_range(), (0.0, 1000.0));
        assert!(anim.is_active(500.0));
        assert!(!anim.is_active(1500.0));
    }

    #[test]
    fn test_fade_on_scroll_range() {
        let anim = ScrollAnimation::FadeOnScroll {
            opacity_from: 0.0,
            opacity_to: 1.0,
            scroll_range: (200.0, 600.0),
            easing: Easing::EaseInOutCubic,
        };

        assert_eq!(anim.scroll_range(), (200.0, 600.0));
        assert!(!anim.is_active(100.0));
        assert!(anim.is_active(400.0));
        assert!(!anim.is_active(700.0));
    }

    #[test]
    fn test_header_state() {
        let state = HeaderState {
            scale: 1.0,
            height: 80.0,
            opacity: 1.0,
        };

        assert_eq!(state.scale, 1.0);
        assert_eq!(state.height, 80.0);
    }
}
