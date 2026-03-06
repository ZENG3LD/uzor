//! Default parameter values for scroll animations
//!
//! Provides sensible defaults based on web animation research and best practices.

use super::types::*;
use crate::easing::Easing;

/// Default parameters for progress bar animations
#[derive(Debug, Clone, Copy)]
pub struct ProgressBarDefaults {
    pub easing: Easing,
    pub scroll_start: f64,
    pub scroll_end: f64,
}

impl Default for ProgressBarDefaults {
    fn default() -> Self {
        Self {
            easing: Easing::Linear,
            scroll_start: 0.0,
            scroll_end: 10000.0,
        }
    }
}

/// Default parameters for parallax layers
#[derive(Debug, Clone)]
pub struct ParallaxDefaults {
    pub layer_speeds: Vec<f64>,
    pub axis: ParallaxAxis,
}

impl Default for ParallaxDefaults {
    fn default() -> Self {
        Self {
            // 3-layer standard: background, midground, foreground
            layer_speeds: vec![0.3, 0.6, 1.0],
            axis: ParallaxAxis::Vertical,
        }
    }
}

/// Default parameters for fade on scroll
#[derive(Debug, Clone, Copy)]
pub struct FadeOnScrollDefaults {
    pub opacity_from: f64,
    pub opacity_to: f64,
    pub easing: Easing,
}

impl Default for FadeOnScrollDefaults {
    fn default() -> Self {
        Self {
            opacity_from: 0.0,
            opacity_to: 1.0,
            easing: Easing::EaseOutCubic,
        }
    }
}

/// Default parameters for reveal on enter
#[derive(Debug, Clone, Copy)]
pub struct RevealOnEnterDefaults {
    pub translate_y: f64,
    pub opacity_from: f64,
    pub duration_ms: u64,
    pub easing: Easing,
    pub entry_start: f64,
    pub entry_end: f64,
}

impl Default for RevealOnEnterDefaults {
    fn default() -> Self {
        Self {
            translate_y: 50.0,
            opacity_from: 0.0,
            duration_ms: 600,
            easing: Easing::EaseOutCubic,
            entry_start: 0.0,
            entry_end: 0.4, // Animate over first 40% of entry
        }
    }
}

/// Default parameters for sticky header
#[derive(Debug, Clone, Copy)]
pub struct StickyHeaderDefaults {
    pub from_height: f64,
    pub to_height: f64,
    pub from_scale: f64,
    pub to_scale: f64,
    pub scroll_threshold: f64,
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for StickyHeaderDefaults {
    fn default() -> Self {
        Self {
            from_height: 80.0,
            to_height: 48.0,
            from_scale: 1.0,
            to_scale: 0.9,
            scroll_threshold: 100.0,
            duration_ms: 300,
            easing: Easing::EaseInOutQuad,
        }
    }
}

/// Default parameters for horizontal scroll
#[derive(Debug, Clone, Copy)]
pub struct HorizontalScrollDefaults {
    pub scroll_distance: f64,
    pub vertical_multiplier: f64, // Vertical scroll range = distance * multiplier
    pub pin: bool,
    pub easing: Easing,
}

impl Default for HorizontalScrollDefaults {
    fn default() -> Self {
        Self {
            scroll_distance: 2000.0,
            vertical_multiplier: 1.5, // 300% vertical scroll for full horizontal
            pin: true,
            easing: Easing::Linear,
        }
    }
}

/// Default parameters for number counter
#[derive(Debug, Clone, Copy)]
pub struct NumberCounterDefaults {
    pub from: f64,
    pub to: f64,
    pub threshold: f64,
    pub duration_ms: u64,
    pub easing: Easing,
}

impl Default for NumberCounterDefaults {
    fn default() -> Self {
        Self {
            from: 0.0,
            to: 100.0,
            threshold: 0.8, // Trigger at 80% visibility
            duration_ms: 2000,
            easing: Easing::EaseOutCubic,
        }
    }
}

/// Default parameters for color shift
#[derive(Debug, Clone)]
pub struct ColorShiftDefaults {
    pub color_stops: Vec<(f64, (f64, f64, f64))>,
    pub easing: Easing,
}

impl Default for ColorShiftDefaults {
    fn default() -> Self {
        Self {
            // Classic blue → purple → pink gradient
            color_stops: vec![
                (0.0, (0.4, 0.47, 0.92)),   // #667EEA
                (0.5, (0.56, 0.27, 0.68)),  // #8E44AD
                (1.0, (0.96, 0.34, 0.42)),  // #F5576C
            ],
            easing: Easing::EaseInOutSine,
        }
    }
}

/// Get default header state for expanded header
pub fn default_header_expanded() -> HeaderState {
    HeaderState {
        scale: 1.0,
        height: 80.0,
        opacity: 1.0,
    }
}

/// Get default header state for collapsed header
pub fn default_header_collapsed() -> HeaderState {
    HeaderState {
        scale: 0.9,
        height: 48.0,
        opacity: 0.95,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_defaults() {
        let defaults = ProgressBarDefaults::default();
        assert_eq!(defaults.easing, Easing::Linear);
        assert_eq!(defaults.scroll_start, 0.0);
    }

    #[test]
    fn test_parallax_defaults() {
        let defaults = ParallaxDefaults::default();
        assert_eq!(defaults.layer_speeds.len(), 3);
        assert_eq!(defaults.axis, ParallaxAxis::Vertical);
    }

    #[test]
    fn test_reveal_on_enter_defaults() {
        let defaults = RevealOnEnterDefaults::default();
        assert_eq!(defaults.translate_y, 50.0);
        assert_eq!(defaults.duration_ms, 600);
    }

    #[test]
    fn test_sticky_header_defaults() {
        let defaults = StickyHeaderDefaults::default();
        assert_eq!(defaults.from_height, 80.0);
        assert_eq!(defaults.to_height, 48.0);
    }

    #[test]
    fn test_number_counter_defaults() {
        let defaults = NumberCounterDefaults::default();
        assert_eq!(defaults.threshold, 0.8);
        assert_eq!(defaults.duration_ms, 2000);
    }

    #[test]
    fn test_header_states() {
        let expanded = default_header_expanded();
        let collapsed = default_header_collapsed();

        assert_eq!(expanded.height, 80.0);
        assert_eq!(collapsed.height, 48.0);
        assert!(expanded.height > collapsed.height);
    }
}
