//! Scroll-linked animations — animations driven by scroll position rather than time
//!
//! Implements CSS Scroll-Driven Animations spec concepts:
//! - ScrollTimeline: Maps scroll range to animation progress
//! - ViewTimeline: Animation based on element visibility in viewport
//! - ScrollTween: Convenience for animating values with scroll
//! - ParallaxLayer: Simple parallax depth effect
//!
//! Unlike time-based animations, scroll-linked animations are deterministic:
//! the same scroll position always produces the same animation state.

use super::easing::Easing;
use super::timeline::Animatable;

/// Defines a scroll range that maps to animation progress 0.0..1.0
///
/// # Example
/// ```
/// use uzor::animation::scroll::ScrollTimeline;
/// use uzor::animation::easing::Easing;
///
/// // Animation starts at scroll position 100px, ends at 500px
/// let timeline = ScrollTimeline::new(100.0, 500.0)
///     .with_easing(Easing::EaseOutCubic);
///
/// assert_eq!(timeline.progress(100.0), 0.0); // Start
/// // At midpoint (300.0), raw progress is 0.5, but EaseOutCubic transforms it to 0.875
/// assert!((timeline.progress(300.0) - 0.875).abs() < 0.01);
/// assert_eq!(timeline.progress(500.0), 1.0); // End
/// ```
#[derive(Debug, Clone)]
pub struct ScrollTimeline {
    /// Scroll position where animation starts (progress = 0.0)
    pub start: f64,
    /// Scroll position where animation ends (progress = 1.0)
    pub end: f64,
    /// Optional easing applied to the progress
    pub easing: Option<Easing>,
    /// Clamp progress to 0..1 or allow overshoot
    pub clamp: bool,
}

impl ScrollTimeline {
    /// Create a new scroll timeline from start to end position
    ///
    /// By default, progress is clamped to 0..1 and uses linear easing.
    pub fn new(start: f64, end: f64) -> Self {
        Self {
            start,
            end,
            easing: None,
            clamp: true,
        }
    }

    /// Set easing function to apply to progress
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = Some(easing);
        self
    }

    /// Allow progress to exceed 0..1 range (disable clamping)
    ///
    /// Useful for spring-like overshoot effects or custom handling.
    pub fn unclamped(mut self) -> Self {
        self.clamp = false;
        self
    }

    /// Compute animation progress from current scroll position
    ///
    /// Returns:
    /// - 0.0 at start position
    /// - 1.0 at end position
    /// - Interpolated value between (with optional easing applied)
    /// - Clamped to 0..1 unless `unclamped()` was called
    pub fn progress(&self, scroll_position: f64) -> f64 {
        // Handle zero-length range (instant transition)
        if (self.end - self.start).abs() < f64::EPSILON {
            return if scroll_position >= self.start {
                1.0
            } else {
                0.0
            };
        }

        // Calculate raw progress
        let raw = (scroll_position - self.start) / (self.end - self.start);

        // Apply clamping if enabled
        let clamped = if self.clamp {
            raw.clamp(0.0, 1.0)
        } else {
            raw
        };

        // Apply easing if specified
        match &self.easing {
            Some(e) => e.ease(clamped),
            None => clamped,
        }
    }

    /// Check if scroll position is within the active range (between start and end)
    pub fn is_active(&self, scroll_position: f64) -> bool {
        let (min, max) = if self.start <= self.end {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };
        scroll_position >= min && scroll_position <= max
    }

    /// Check if animation is complete (scroll position at or past end)
    pub fn is_complete(&self, scroll_position: f64) -> bool {
        if self.start <= self.end {
            scroll_position >= self.end
        } else {
            scroll_position <= self.end
        }
    }
}

/// Animation driven by element visibility in viewport
///
/// Useful for "reveal on scroll" animations. Progress is calculated based on
/// how much of an element is visible in the viewport.
///
/// # Example
/// ```
/// use uzor::animation::scroll::ViewTimeline;
///
/// // Element from y=500 to y=800 (300px tall)
/// // Viewport is 1000px tall
/// let timeline = ViewTimeline::new(500.0, 800.0, 1000.0)
///     .with_thresholds(0.0, 1.0);
///
/// // Element starts entering viewport when viewport_bottom reaches element_top
/// // Element is fully entered when fully visible
/// ```
#[derive(Debug, Clone)]
pub struct ViewTimeline {
    /// Element's top position (absolute, in scroll content coordinates)
    pub element_top: f64,
    /// Element's bottom position
    pub element_bottom: f64,
    /// Viewport height
    pub viewport_height: f64,
    /// When to start: 0.0 = element enters bottom edge, 1.0 = element fully visible
    pub start_threshold: f64,
    /// When to end: 0.0 = element starts leaving, 1.0 = element fully exited
    pub end_threshold: f64,
    /// Optional easing
    pub easing: Option<Easing>,
}

impl ViewTimeline {
    /// Create a new view timeline
    ///
    /// # Arguments
    /// - `element_top`: Y position of element's top edge (in scroll coordinates)
    /// - `element_bottom`: Y position of element's bottom edge
    /// - `viewport_height`: Height of the viewport
    ///
    /// Default behavior:
    /// - Animation starts when element enters viewport (bottom edge)
    /// - Animation completes when element is fully visible
    pub fn new(element_top: f64, element_bottom: f64, viewport_height: f64) -> Self {
        Self {
            element_top,
            element_bottom,
            viewport_height,
            start_threshold: 0.0,
            end_threshold: 1.0,
            easing: None,
        }
    }

    /// Set visibility thresholds for animation start/end
    ///
    /// # Arguments
    /// - `start`: 0.0 = element enters viewport, 1.0 = element fully visible
    /// - `end`: 0.0 = element starts exiting, 1.0 = element fully exited
    pub fn with_thresholds(mut self, start: f64, end: f64) -> Self {
        self.start_threshold = start.clamp(0.0, 1.0);
        self.end_threshold = end.clamp(0.0, 1.0);
        self
    }

    /// Set easing function for progress
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = Some(easing);
        self
    }

    /// Compute animation progress from scroll position (viewport top edge)
    ///
    /// # Algorithm
    /// ```text
    /// viewport_bottom = scroll_top + viewport_height
    /// element_height = element_bottom - element_top
    ///
    /// // Element enters viewport when viewport_bottom reaches element_top
    /// enter_point = element_top - viewport_height
    ///
    /// // Element is "fully entered" based on start_threshold
    /// // 0.0 = just entered, 1.0 = fully visible
    /// start_point = enter_point + (viewport_height + element_height) * start_threshold
    ///
    /// // Element exits based on end_threshold
    /// end_point = enter_point + (viewport_height + element_height) * end_threshold
    /// ```
    pub fn progress(&self, scroll_top: f64) -> f64 {
        let element_height = self.element_bottom - self.element_top;

        // Point where element first enters viewport (viewport_bottom = element_top)
        let enter_point = self.element_top - self.viewport_height;

        // Total travel distance (element enters to fully exits viewport)
        let total_travel = self.viewport_height + element_height;

        // Calculate animation start and end points based on thresholds
        let start_point = enter_point + total_travel * self.start_threshold;
        let end_point = enter_point + total_travel * self.end_threshold;

        // Avoid division by zero
        if (end_point - start_point).abs() < f64::EPSILON {
            return if scroll_top >= start_point { 1.0 } else { 0.0 };
        }

        // Calculate raw progress
        let raw = (scroll_top - start_point) / (end_point - start_point);
        let clamped = raw.clamp(0.0, 1.0);

        // Apply easing if specified
        match &self.easing {
            Some(e) => e.ease(clamped),
            None => clamped,
        }
    }

    /// Check if element is currently visible in viewport
    pub fn is_visible(&self, scroll_top: f64) -> bool {
        let viewport_bottom = scroll_top + self.viewport_height;
        // Element is visible if it overlaps with viewport
        self.element_bottom > scroll_top && self.element_top < viewport_bottom
    }

    /// Check if element has completely entered viewport (fully visible)
    pub fn is_fully_visible(&self, scroll_top: f64) -> bool {
        let viewport_bottom = scroll_top + self.viewport_height;
        self.element_top >= scroll_top && self.element_bottom <= viewport_bottom
    }

    /// Check if element has started exiting viewport
    pub fn is_exiting(&self, scroll_top: f64) -> bool {
        self.element_top < scroll_top || self.element_bottom > scroll_top + self.viewport_height
    }
}

/// Animate a value from start to end based on scroll progress
///
/// Convenience wrapper combining a value interpolation with a ScrollTimeline.
///
/// # Example
/// ```
/// use uzor::animation::scroll::{ScrollTween, ScrollTimeline};
///
/// // Fade opacity from 0.0 to 1.0 as user scrolls from 100px to 500px
/// let timeline = ScrollTimeline::new(100.0, 500.0);
/// let tween = ScrollTween::new(0.0, 1.0, timeline);
///
/// assert_eq!(tween.value_at(100.0), 0.0);
/// assert_eq!(tween.value_at(300.0), 0.5);
/// assert_eq!(tween.value_at(500.0), 1.0);
/// ```
#[derive(Debug, Clone)]
pub struct ScrollTween<T: Animatable> {
    pub from: T,
    pub to: T,
    pub timeline: ScrollTimeline,
}

impl<T: Animatable> ScrollTween<T> {
    /// Create a new scroll-linked tween
    pub fn new(from: T, to: T, timeline: ScrollTimeline) -> Self {
        Self { from, to, timeline }
    }

    /// Get the animated value at current scroll position
    pub fn value_at(&self, scroll_position: f64) -> T {
        let progress = self.timeline.progress(scroll_position);
        self.from.lerp(&self.to, progress)
    }

    /// Check if animation is active at this scroll position
    pub fn is_active(&self, scroll_position: f64) -> bool {
        self.timeline.is_active(scroll_position)
    }

    /// Check if animation is complete at this scroll position
    pub fn is_complete(&self, scroll_position: f64) -> bool {
        self.timeline.is_complete(scroll_position)
    }
}

/// Simple parallax effect based on depth factor
///
/// Slower layers (lower depth) move less than faster layers (higher depth).
/// Depth factor of 0.0 means static (background), 1.0 means moves with scroll (foreground).
///
/// # Example
/// ```
/// use uzor::animation::scroll::ParallaxLayer;
///
/// let background = ParallaxLayer::new(0.0);    // Static
/// let midground = ParallaxLayer::new(0.5);     // Half speed
/// let foreground = ParallaxLayer::new(1.0);    // Full speed
///
/// let scroll = 100.0;
/// assert_eq!(background.offset(scroll), 0.0);      // No movement
/// assert_eq!(midground.offset(scroll), 50.0);      // Moves at half speed
/// assert_eq!(foreground.offset(scroll), 100.0);    // Moves at full speed
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ParallaxLayer {
    /// Depth factor: 0.0 = static (far background), 1.0 = foreground (moves with scroll)
    pub depth: f64,
}

impl ParallaxLayer {
    /// Create a new parallax layer with given depth factor
    ///
    /// Typical values:
    /// - 0.0: Static background
    /// - 0.3: Far layer
    /// - 0.5: Mid layer
    /// - 0.7: Near layer
    /// - 1.0: Foreground (moves exactly with scroll)
    pub fn new(depth: f64) -> Self {
        Self {
            depth: depth.clamp(0.0, 1.0),
        }
    }

    /// Compute offset for this layer given scroll position
    ///
    /// Returns the amount this layer should be offset based on scroll.
    /// Lower depth values = less movement = "further away" appearance.
    pub fn offset(&self, scroll_position: f64) -> f64 {
        scroll_position * self.depth
    }

    /// Compute offset relative to a reference scroll position
    ///
    /// Useful when you want parallax relative to a specific anchor point
    /// rather than absolute scroll position.
    pub fn offset_relative(&self, scroll_position: f64, reference: f64) -> f64 {
        (scroll_position - reference) * self.depth
    }

    /// Set depth factor (clamped to 0..1)
    pub fn with_depth(mut self, depth: f64) -> Self {
        self.depth = depth.clamp(0.0, 1.0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_timeline_basic() {
        let timeline = ScrollTimeline::new(0.0, 100.0);

        assert_eq!(timeline.progress(0.0), 0.0);
        assert_eq!(timeline.progress(50.0), 0.5);
        assert_eq!(timeline.progress(100.0), 1.0);
    }

    #[test]
    fn test_scroll_timeline_clamped() {
        let timeline = ScrollTimeline::new(100.0, 200.0);

        // Before start
        assert_eq!(timeline.progress(0.0), 0.0);
        // After end
        assert_eq!(timeline.progress(300.0), 1.0);
        // Middle
        assert_eq!(timeline.progress(150.0), 0.5);
    }

    #[test]
    fn test_scroll_timeline_unclamped() {
        let timeline = ScrollTimeline::new(100.0, 200.0).unclamped();

        // Before start - negative progress
        assert_eq!(timeline.progress(0.0), -1.0);
        // After end - progress > 1
        assert_eq!(timeline.progress(300.0), 2.0);
        // Middle
        assert_eq!(timeline.progress(150.0), 0.5);
    }

    #[test]
    fn test_scroll_timeline_with_easing() {
        let timeline = ScrollTimeline::new(0.0, 100.0).with_easing(Easing::EaseInQuad);

        assert_eq!(timeline.progress(0.0), 0.0);
        assert_eq!(timeline.progress(50.0), 0.25); // EaseInQuad: t^2
        assert_eq!(timeline.progress(100.0), 1.0);
    }

    #[test]
    fn test_scroll_timeline_zero_length() {
        let timeline = ScrollTimeline::new(100.0, 100.0);

        // Before start
        assert_eq!(timeline.progress(99.0), 0.0);
        // At or after start
        assert_eq!(timeline.progress(100.0), 1.0);
        assert_eq!(timeline.progress(101.0), 1.0);
    }

    #[test]
    fn test_scroll_timeline_is_active() {
        let timeline = ScrollTimeline::new(100.0, 200.0);

        assert!(!timeline.is_active(50.0));
        assert!(timeline.is_active(100.0));
        assert!(timeline.is_active(150.0));
        assert!(timeline.is_active(200.0));
        assert!(!timeline.is_active(250.0));
    }

    #[test]
    fn test_scroll_timeline_is_complete() {
        let timeline = ScrollTimeline::new(100.0, 200.0);

        assert!(!timeline.is_complete(50.0));
        assert!(!timeline.is_complete(100.0));
        assert!(!timeline.is_complete(150.0));
        assert!(timeline.is_complete(200.0));
        assert!(timeline.is_complete(250.0));
    }

    #[test]
    fn test_scroll_timeline_reverse() {
        // Timeline can go backwards too
        let timeline = ScrollTimeline::new(200.0, 100.0);

        assert_eq!(timeline.progress(200.0), 0.0);
        assert_eq!(timeline.progress(150.0), 0.5);
        assert_eq!(timeline.progress(100.0), 1.0);
    }

    #[test]
    fn test_view_timeline_basic() {
        // Element from 500 to 800 (300px tall)
        // Viewport 1000px tall
        let timeline = ViewTimeline::new(500.0, 800.0, 1000.0);

        // Element enters viewport when viewport_bottom = element_top = 500
        // This happens when scroll_top = 500 - 1000 = -500
        let enter_scroll = -500.0;

        // Element fully exits viewport when scroll_top = 800 (element_bottom = scroll_top)
        let exit_scroll = 800.0;

        // At enter point (with default start_threshold=0.0), progress should be 0.0
        let progress_at_enter = timeline.progress(enter_scroll);
        assert!(progress_at_enter < 0.01); // Should be very close to 0

        // At exit point (with default end_threshold=1.0), progress should be 1.0
        let progress_at_exit = timeline.progress(exit_scroll);
        assert!(progress_at_exit > 0.99); // Should be very close to 1

        // Midpoint should be around 0.5
        let midpoint = (enter_scroll + exit_scroll) / 2.0;
        let progress_mid = timeline.progress(midpoint);
        assert!((progress_mid - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_view_timeline_is_visible() {
        let timeline = ViewTimeline::new(500.0, 800.0, 1000.0);

        // Not visible when viewport is above element
        assert!(!timeline.is_visible(-600.0));

        // Visible when viewport overlaps
        assert!(timeline.is_visible(0.0));
        assert!(timeline.is_visible(500.0));

        // Not visible when viewport is below element
        assert!(!timeline.is_visible(1000.0));
    }

    #[test]
    fn test_view_timeline_is_fully_visible() {
        let timeline = ViewTimeline::new(500.0, 800.0, 1000.0);

        // Element is 300px tall, viewport is 1000px tall
        // Element is fully visible when viewport_top <= 500 AND viewport_bottom >= 800
        // viewport_bottom = scroll_top + 1000
        // So: scroll_top <= 500 AND scroll_top + 1000 >= 800
        // => scroll_top <= 500 AND scroll_top >= -200
        // => -200 <= scroll_top <= 500

        assert!(!timeline.is_fully_visible(-300.0)); // Viewport above
        assert!(timeline.is_fully_visible(0.0)); // Fully visible
        assert!(timeline.is_fully_visible(500.0)); // Fully visible
        assert!(!timeline.is_fully_visible(600.0)); // Viewport below
    }

    #[test]
    fn test_scroll_tween() {
        let timeline = ScrollTimeline::new(0.0, 100.0);
        let tween = ScrollTween::new(0.0, 1.0, timeline);

        assert_eq!(tween.value_at(0.0), 0.0);
        assert_eq!(tween.value_at(50.0), 0.5);
        assert_eq!(tween.value_at(100.0), 1.0);
    }

    #[test]
    fn test_scroll_tween_with_tuples() {
        let timeline = ScrollTimeline::new(0.0, 100.0);
        let tween = ScrollTween::new((0.0, 0.0), (100.0, 200.0), timeline);

        assert_eq!(tween.value_at(0.0), (0.0, 0.0));
        assert_eq!(tween.value_at(50.0), (50.0, 100.0));
        assert_eq!(tween.value_at(100.0), (100.0, 200.0));
    }

    #[test]
    fn test_parallax_layer_basic() {
        let background = ParallaxLayer::new(0.0);
        let midground = ParallaxLayer::new(0.5);
        let foreground = ParallaxLayer::new(1.0);

        let scroll = 100.0;

        assert_eq!(background.offset(scroll), 0.0);
        assert_eq!(midground.offset(scroll), 50.0);
        assert_eq!(foreground.offset(scroll), 100.0);
    }

    #[test]
    fn test_parallax_layer_relative() {
        let layer = ParallaxLayer::new(0.5);

        let reference = 100.0;
        assert_eq!(layer.offset_relative(100.0, reference), 0.0);
        assert_eq!(layer.offset_relative(200.0, reference), 50.0);
        assert_eq!(layer.offset_relative(0.0, reference), -50.0);
    }

    #[test]
    fn test_parallax_layer_clamping() {
        // Depth values should be clamped to 0..1
        let too_low = ParallaxLayer::new(-0.5);
        let too_high = ParallaxLayer::new(1.5);

        assert_eq!(too_low.depth, 0.0);
        assert_eq!(too_high.depth, 1.0);
    }

    #[test]
    fn test_parallax_layer_zero_scroll() {
        let layer = ParallaxLayer::new(0.5);
        assert_eq!(layer.offset(0.0), 0.0);
    }
}
