//! Builder pattern for scroll animations
//!
//! Provides fluent API for constructing scroll animations with custom parameters.

use super::types::*;
use super::defaults::*;
use crate::easing::Easing;
use std::time::Duration;

/// Builder for progress bar animations
pub struct ProgressBarBuilder {
    scroll_start: f64,
    scroll_end: f64,
    easing: Easing,
    orientation: ProgressBarOrientation,
}

impl ProgressBarBuilder {
    pub fn new() -> Self {
        let defaults = ProgressBarDefaults::default();
        Self {
            scroll_start: defaults.scroll_start,
            scroll_end: defaults.scroll_end,
            easing: defaults.easing,
            orientation: ProgressBarOrientation::Horizontal,
        }
    }

    pub fn scroll_start(mut self, start: f64) -> Self {
        self.scroll_start = start;
        self
    }

    pub fn scroll_end(mut self, end: f64) -> Self {
        self.scroll_end = end;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn orientation(mut self, orientation: ProgressBarOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::ProgressBar {
            scroll_start: self.scroll_start,
            scroll_end: self.scroll_end,
            easing: self.easing,
            orientation: self.orientation,
        }
    }
}

impl Default for ProgressBarBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for parallax layer animations
pub struct ParallaxLayersBuilder {
    layer_speeds: Vec<f64>,
    axis: ParallaxAxis,
}

impl ParallaxLayersBuilder {
    pub fn new() -> Self {
        let defaults = ParallaxDefaults::default();
        Self {
            layer_speeds: defaults.layer_speeds,
            axis: defaults.axis,
        }
    }

    pub fn layer_speeds(mut self, speeds: Vec<f64>) -> Self {
        self.layer_speeds = speeds;
        self
    }

    pub fn add_layer(mut self, speed: f64) -> Self {
        self.layer_speeds.push(speed);
        self
    }

    pub fn axis(mut self, axis: ParallaxAxis) -> Self {
        self.axis = axis;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::ParallaxLayers {
            layer_speeds: self.layer_speeds,
            axis: self.axis,
        }
    }
}

impl Default for ParallaxLayersBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for fade on scroll animations
pub struct FadeOnScrollBuilder {
    opacity_from: f64,
    opacity_to: f64,
    scroll_range: (f64, f64),
    easing: Easing,
}

impl FadeOnScrollBuilder {
    pub fn new() -> Self {
        let defaults = FadeOnScrollDefaults::default();
        Self {
            opacity_from: defaults.opacity_from,
            opacity_to: defaults.opacity_to,
            scroll_range: (0.0, 1000.0),
            easing: defaults.easing,
        }
    }

    pub fn opacity_from(mut self, opacity: f64) -> Self {
        self.opacity_from = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn opacity_to(mut self, opacity: f64) -> Self {
        self.opacity_to = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn scroll_range(mut self, start: f64, end: f64) -> Self {
        self.scroll_range = (start, end);
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::FadeOnScroll {
            opacity_from: self.opacity_from,
            opacity_to: self.opacity_to,
            scroll_range: self.scroll_range,
            easing: self.easing,
        }
    }
}

impl Default for FadeOnScrollBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for reveal on enter animations
pub struct RevealOnEnterBuilder {
    translate_y: f64,
    opacity_from: f64,
    entry_range: (f64, f64),
    duration: Duration,
    easing: Easing,
}

impl RevealOnEnterBuilder {
    pub fn new() -> Self {
        let defaults = RevealOnEnterDefaults::default();
        Self {
            translate_y: defaults.translate_y,
            opacity_from: defaults.opacity_from,
            entry_range: (defaults.entry_start, defaults.entry_end),
            duration: Duration::from_millis(defaults.duration_ms),
            easing: defaults.easing,
        }
    }

    pub fn translate_y(mut self, distance: f64) -> Self {
        self.translate_y = distance;
        self
    }

    pub fn opacity_from(mut self, opacity: f64) -> Self {
        self.opacity_from = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn entry_range(mut self, start: f64, end: f64) -> Self {
        self.entry_range = (start, end);
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::RevealOnEnter {
            translate_y: self.translate_y,
            opacity_from: self.opacity_from,
            entry_range: self.entry_range,
            duration: self.duration,
            easing: self.easing,
        }
    }
}

impl Default for RevealOnEnterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for sticky header animations
pub struct StickyHeaderBuilder {
    from_state: HeaderState,
    to_state: HeaderState,
    scroll_threshold: f64,
    duration: Duration,
    easing: Easing,
}

impl StickyHeaderBuilder {
    pub fn new() -> Self {
        let defaults = StickyHeaderDefaults::default();
        Self {
            from_state: default_header_expanded(),
            to_state: default_header_collapsed(),
            scroll_threshold: defaults.scroll_threshold,
            duration: Duration::from_millis(defaults.duration_ms),
            easing: defaults.easing,
        }
    }

    pub fn from_state(mut self, state: HeaderState) -> Self {
        self.from_state = state;
        self
    }

    pub fn to_state(mut self, state: HeaderState) -> Self {
        self.to_state = state;
        self
    }

    pub fn scroll_threshold(mut self, threshold: f64) -> Self {
        self.scroll_threshold = threshold;
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::StickyHeader {
            from_state: self.from_state,
            to_state: self.to_state,
            scroll_threshold: self.scroll_threshold,
            duration: self.duration,
            easing: self.easing,
        }
    }
}

impl Default for StickyHeaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for horizontal scroll animations
pub struct HorizontalScrollBuilder {
    scroll_distance: f64,
    vertical_range: (f64, f64),
    pin: bool,
    easing: Easing,
}

impl HorizontalScrollBuilder {
    pub fn new() -> Self {
        let defaults = HorizontalScrollDefaults::default();
        Self {
            scroll_distance: defaults.scroll_distance,
            vertical_range: (0.0, defaults.scroll_distance * defaults.vertical_multiplier),
            pin: defaults.pin,
            easing: defaults.easing,
        }
    }

    pub fn scroll_distance(mut self, distance: f64) -> Self {
        self.scroll_distance = distance;
        self
    }

    pub fn vertical_range(mut self, start: f64, end: f64) -> Self {
        self.vertical_range = (start, end);
        self
    }

    pub fn pin(mut self, pin: bool) -> Self {
        self.pin = pin;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::HorizontalScroll {
            scroll_distance: self.scroll_distance,
            vertical_range: self.vertical_range,
            pin: self.pin,
            easing: self.easing,
        }
    }
}

impl Default for HorizontalScrollBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for number counter animations
pub struct NumberCounterBuilder {
    from: f64,
    to: f64,
    threshold: f64,
    duration: Duration,
    easing: Easing,
}

impl NumberCounterBuilder {
    pub fn new() -> Self {
        let defaults = NumberCounterDefaults::default();
        Self {
            from: defaults.from,
            to: defaults.to,
            threshold: defaults.threshold,
            duration: Duration::from_millis(defaults.duration_ms),
            easing: defaults.easing,
        }
    }

    pub fn from(mut self, value: f64) -> Self {
        self.from = value;
        self
    }

    pub fn to(mut self, value: f64) -> Self {
        self.to = value;
        self
    }

    pub fn threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::NumberCounter {
            from: self.from,
            to: self.to,
            threshold: self.threshold,
            duration: self.duration,
            easing: self.easing,
        }
    }
}

impl Default for NumberCounterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for color shift animations
pub struct ColorShiftBuilder {
    color_stops: Vec<(f64, (f64, f64, f64))>,
    scroll_range: (f64, f64),
    easing: Easing,
}

impl ColorShiftBuilder {
    pub fn new() -> Self {
        let defaults = ColorShiftDefaults::default();
        Self {
            color_stops: defaults.color_stops,
            scroll_range: (0.0, 5000.0),
            easing: defaults.easing,
        }
    }

    pub fn color_stops(mut self, stops: Vec<(f64, (f64, f64, f64))>) -> Self {
        self.color_stops = stops;
        self
    }

    pub fn add_stop(mut self, progress: f64, color: (f64, f64, f64)) -> Self {
        self.color_stops.push((progress, color));
        self
    }

    pub fn scroll_range(mut self, start: f64, end: f64) -> Self {
        self.scroll_range = (start, end);
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn build(self) -> ScrollAnimation {
        ScrollAnimation::ColorShift {
            color_stops: self.color_stops,
            scroll_range: self.scroll_range,
            easing: self.easing,
        }
    }
}

impl Default for ColorShiftBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar_builder() {
        let anim = ProgressBarBuilder::new()
            .scroll_start(100.0)
            .scroll_end(2000.0)
            .orientation(ProgressBarOrientation::Vertical)
            .build();

        match anim {
            ScrollAnimation::ProgressBar {
                scroll_start,
                scroll_end,
                orientation,
                ..
            } => {
                assert_eq!(scroll_start, 100.0);
                assert_eq!(scroll_end, 2000.0);
                assert_eq!(orientation, ProgressBarOrientation::Vertical);
            }
            _ => panic!("Expected ProgressBar variant"),
        }
    }

    #[test]
    fn test_parallax_builder() {
        let anim = ParallaxLayersBuilder::new()
            .layer_speeds(vec![0.2, 0.5, 0.8])
            .axis(ParallaxAxis::Horizontal)
            .build();

        match anim {
            ScrollAnimation::ParallaxLayers {
                layer_speeds, axis, ..
            } => {
                assert_eq!(layer_speeds.len(), 3);
                assert_eq!(axis, ParallaxAxis::Horizontal);
            }
            _ => panic!("Expected ParallaxLayers variant"),
        }
    }

    #[test]
    fn test_fade_builder() {
        let anim = FadeOnScrollBuilder::new()
            .opacity_from(0.2)
            .opacity_to(0.9)
            .scroll_range(500.0, 1500.0)
            .build();

        match anim {
            ScrollAnimation::FadeOnScroll {
                opacity_from,
                opacity_to,
                scroll_range,
                ..
            } => {
                assert_eq!(opacity_from, 0.2);
                assert_eq!(opacity_to, 0.9);
                assert_eq!(scroll_range, (500.0, 1500.0));
            }
            _ => panic!("Expected FadeOnScroll variant"),
        }
    }

    #[test]
    fn test_reveal_builder() {
        let anim = RevealOnEnterBuilder::new()
            .translate_y(100.0)
            .duration(Duration::from_millis(800))
            .build();

        match anim {
            ScrollAnimation::RevealOnEnter {
                translate_y,
                duration,
                ..
            } => {
                assert_eq!(translate_y, 100.0);
                assert_eq!(duration, Duration::from_millis(800));
            }
            _ => panic!("Expected RevealOnEnter variant"),
        }
    }

    #[test]
    fn test_number_counter_builder() {
        let anim = NumberCounterBuilder::new()
            .from(10.0)
            .to(500.0)
            .threshold(0.5)
            .build();

        match anim {
            ScrollAnimation::NumberCounter {
                from, to, threshold, ..
            } => {
                assert_eq!(from, 10.0);
                assert_eq!(to, 500.0);
                assert_eq!(threshold, 0.5);
            }
            _ => panic!("Expected NumberCounter variant"),
        }
    }

    #[test]
    fn test_color_shift_builder() {
        let anim = ColorShiftBuilder::new()
            .add_stop(0.0, (1.0, 0.0, 0.0))
            .add_stop(1.0, (0.0, 0.0, 1.0))
            .scroll_range(0.0, 3000.0)
            .build();

        match anim {
            ScrollAnimation::ColorShift {
                color_stops,
                scroll_range,
                ..
            } => {
                // Default has 3 stops + 2 added = 5 total
                assert!(color_stops.len() >= 2);
                assert_eq!(scroll_range, (0.0, 3000.0));
            }
            _ => panic!("Expected ColorShift variant"),
        }
    }
}
