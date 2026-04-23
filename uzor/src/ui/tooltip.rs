//! Tooltip system for widget hover information
//!
//! Provides a centralized system for managing tooltips that appear after hovering
//! over widgets for a configurable delay period, with fade-in animation and
//! flexible visual theming.

use crate::types::WidgetId;

/// Theme trait for tooltip visual styling.
///
/// Applications implement this to customize tooltip appearance without
/// coupling to any specific rendering backend.
pub trait TooltipTheme {
    fn background_color(&self) -> &str;
    fn text_color(&self) -> &str;
    fn border_color(&self) -> &str;
    fn border_width(&self) -> f64;
    fn corner_radius(&self) -> f64;
    fn padding(&self) -> f64;
    fn font_size(&self) -> f64;
    fn shadow_color(&self) -> &str;
    fn shadow_blur(&self) -> f64;
    fn shadow_offset(&self) -> (f64, f64);
}

/// Default dark tooltip theme matching common dark-UI conventions.
pub struct DefaultTooltipTheme;

impl TooltipTheme for DefaultTooltipTheme {
    fn background_color(&self) -> &str { "#323232" }
    fn text_color(&self) -> &str { "#ffffff" }
    fn border_color(&self) -> &str { "#505050" }
    fn border_width(&self) -> f64 { 1.0 }
    fn corner_radius(&self) -> f64 { 4.0 }
    fn padding(&self) -> f64 { 6.0 }
    fn font_size(&self) -> f64 { 12.0 }
    fn shadow_color(&self) -> &str { "#00000060" }
    fn shadow_blur(&self) -> f64 { 4.0 }
    fn shadow_offset(&self) -> (f64, f64) { (0.0, 2.0) }
}

/// Request to show a tooltip
#[derive(Clone, Debug)]
pub struct TooltipRequest {
    /// Tooltip text content
    pub text: String,
    /// Position to show tooltip (usually near cursor)
    pub position: (f64, f64),
    /// Widget that requested the tooltip
    pub widget_id: WidgetId,
    /// Time when hover started
    pub hover_start_time: f64,
}

/// Configuration for tooltip behavior and appearance
#[derive(Clone, Debug)]
pub struct TooltipConfig {
    /// Delay before showing (ms)
    pub show_delay_ms: f64,
    /// Offset from cursor
    pub offset: (f64, f64),
    /// Max width before wrapping
    pub max_width: f64,
    /// Duration of the fade-in animation (ms)
    pub fade_in_duration_ms: f64,
    /// Corner radius for rounded tooltip box
    pub corner_radius: f64,
    /// Font size used for size estimation
    pub font_size: f64,
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self {
            show_delay_ms: 500.0,
            offset: (10.0, 10.0),
            max_width: 300.0,
            fade_in_duration_ms: 150.0,
            corner_radius: 4.0,
            font_size: 12.0,
        }
    }
}

/// Manages tooltip display state including fade-in opacity
#[derive(Clone, Debug, Default)]
pub struct TooltipState {
    /// Current active tooltip request
    active: Option<TooltipRequest>,
    /// Currently hovered widget
    hovered_widget: Option<WidgetId>,
    /// Time when hover started on current widget
    hover_start: f64,
    /// Delay before showing tooltip (ms)
    show_delay_ms: f64,
    /// Whether tooltip is currently visible
    visible: bool,
    /// Timestamp when tooltip became visible (for fade calculation)
    visible_start: f64,
    /// Fade-in duration in milliseconds
    fade_in_duration_ms: f64,
    /// Current fade opacity [0.0, 1.0]
    fade_opacity: f64,
}

impl TooltipState {
    /// Create new tooltip state with default 500ms delay and 150ms fade
    pub fn new() -> Self {
        Self {
            active: None,
            hovered_widget: None,
            hover_start: 0.0,
            show_delay_ms: 500.0,
            visible: false,
            visible_start: 0.0,
            fade_in_duration_ms: 150.0,
            fade_opacity: 0.0,
        }
    }

    /// Create new tooltip state with custom show delay
    pub fn with_delay(delay_ms: f64) -> Self {
        Self {
            show_delay_ms: delay_ms,
            ..Self::new()
        }
    }

    /// Create new tooltip state from a full config
    pub fn with_config(config: &TooltipConfig) -> Self {
        Self {
            show_delay_ms: config.show_delay_ms,
            fade_in_duration_ms: config.fade_in_duration_ms,
            ..Self::new()
        }
    }

    /// Set the tooltip show delay
    pub fn set_delay(&mut self, delay_ms: f64) {
        self.show_delay_ms = delay_ms;
    }

    /// Set the fade-in duration
    pub fn set_fade_duration(&mut self, fade_ms: f64) {
        self.fade_in_duration_ms = fade_ms;
    }

    /// Update tooltip state based on currently hovered widget.
    ///
    /// Call this each frame with the widget currently under the cursor.
    /// When the hovered widget changes, the hover timer resets.
    pub fn update(&mut self, hovered_widget: Option<WidgetId>, time: f64) {
        match (&self.hovered_widget, &hovered_widget) {
            // Widget changed — reset hover timer and hide tooltip
            (Some(old_id), Some(new_id)) if old_id != new_id => {
                self.hovered_widget = Some(new_id.clone());
                self.hover_start = time;
                self.visible = false;
                self.visible_start = 0.0;
                self.fade_opacity = 0.0;
                self.active = None;
            }
            // Started hovering a widget
            (None, Some(new_id)) => {
                self.hovered_widget = Some(new_id.clone());
                self.hover_start = time;
                self.visible = false;
                self.visible_start = 0.0;
                self.fade_opacity = 0.0;
            }
            // Stopped hovering
            (Some(_), None) => {
                self.hovered_widget = None;
                self.visible = false;
                self.visible_start = 0.0;
                self.fade_opacity = 0.0;
                self.active = None;
            }
            // Same widget or still no hover — keep current state
            _ => {}
        }

        // Transition to visible once delay has elapsed
        if self.hovered_widget.is_some() && !self.visible && self.should_show(time) {
            self.visible = true;
            self.visible_start = time;
        }

        // Update fade opacity while visible
        if self.visible {
            self.fade_opacity = calculate_fade_opacity(
                time - self.visible_start,
                self.fade_in_duration_ms,
            );
        }
    }

    /// Request a tooltip for a specific widget.
    ///
    /// Should be called each frame by any widget that wants a tooltip while hovered.
    /// The tooltip is only shown after the configured delay has elapsed.
    pub fn request_tooltip(
        &mut self,
        widget_id: WidgetId,
        text: String,
        pos: (f64, f64),
        time: f64,
    ) {
        // Only accept request if this widget is currently hovered
        if let Some(ref hovered) = self.hovered_widget {
            if hovered == &widget_id {
                self.active = Some(TooltipRequest {
                    text,
                    position: pos,
                    widget_id,
                    hover_start_time: self.hover_start,
                });

                // Update visibility based on delay
                if !self.visible && self.should_show(time) {
                    self.visible = true;
                    self.visible_start = time;
                }
                if self.visible {
                    self.fade_opacity = calculate_fade_opacity(
                        time - self.visible_start,
                        self.fade_in_duration_ms,
                    );
                }
            }
        }
    }

    /// Check if enough time has passed to show the tooltip
    pub fn should_show(&self, time: f64) -> bool {
        if self.hovered_widget.is_none() {
            return false;
        }
        (time - self.hover_start) >= self.show_delay_ms
    }

    /// Get the active tooltip if it should be visible
    pub fn get_active(&self) -> Option<&TooltipRequest> {
        if self.visible {
            self.active.as_ref()
        } else {
            None
        }
    }

    /// Clear and hide the tooltip
    pub fn clear(&mut self) {
        self.visible = false;
        self.visible_start = 0.0;
        self.fade_opacity = 0.0;
        self.active = None;
        self.hovered_widget = None;
    }

    /// Check if tooltip is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the currently hovered widget ID
    pub fn hovered_widget(&self) -> Option<&WidgetId> {
        self.hovered_widget.as_ref()
    }

    /// Get the current fade opacity in the range [0.0, 1.0].
    ///
    /// Returns 0.0 when not visible, linearly interpolates to 1.0 over
    /// `fade_in_duration_ms` after the tooltip becomes visible.
    pub fn get_opacity(&self) -> f64 {
        if self.visible {
            self.fade_opacity
        } else {
            0.0
        }
    }
}

/// Linearly interpolate fade opacity from 0→1 over `fade_duration_ms`.
///
/// Returns 1.0 once `elapsed_ms >= fade_duration_ms`.
fn calculate_fade_opacity(elapsed_ms: f64, fade_duration_ms: f64) -> f64 {
    if fade_duration_ms <= 0.0 || elapsed_ms >= fade_duration_ms {
        1.0
    } else {
        elapsed_ms / fade_duration_ms
    }
}

/// Calculate tooltip position near cursor, avoiding screen edges.
///
/// Uses a flip-then-clamp strategy: first tries to place the tooltip at
/// `cursor + offset`; if that would overflow an edge, flips to the opposite
/// side; if still off-screen, clamps to the edge.
///
/// # Arguments
/// * `cursor` - Current cursor position (x, y)
/// * `tooltip_size` - Size of the tooltip (width, height)
/// * `screen_size` - Size of the screen/viewport (width, height)
/// * `offset` - Desired offset from cursor (x, y)
///
/// # Returns
/// Position (x, y) where the tooltip should be drawn
pub fn calculate_tooltip_position(
    cursor: (f64, f64),
    tooltip_size: (f64, f64),
    screen_size: (f64, f64),
    offset: (f64, f64),
) -> (f64, f64) {
    let mut x = cursor.0 + offset.0;
    let mut y = cursor.1 + offset.1;

    // Check right edge
    if x + tooltip_size.0 > screen_size.0 {
        // Try positioning to the left of cursor instead
        x = cursor.0 - tooltip_size.0 - offset.0;
        // If still off screen, clamp to edge
        if x < 0.0 {
            x = screen_size.0 - tooltip_size.0;
        }
    }

    // Check left edge
    if x < 0.0 {
        x = 0.0;
    }

    // Check bottom edge
    if y + tooltip_size.1 > screen_size.1 {
        // Try positioning above cursor instead
        y = cursor.1 - tooltip_size.1 - offset.1;
        // If still off screen, clamp to edge
        if y < 0.0 {
            y = screen_size.1 - tooltip_size.1;
        }
    }

    // Check top edge
    if y < 0.0 {
        y = 0.0;
    }

    (x, y)
}

/// Estimate tooltip size based on text content.
///
/// Uses a character-width heuristic (`font_size * 0.6` per character) to
/// approximate multi-line layout within `max_width`. Suitable for positioning
/// calculations before actual rendering.
///
/// # Arguments
/// * `text` - Tooltip text content
/// * `max_width` - Maximum allowed width in pixels
/// * `font_size` - Font size in pixels
///
/// # Returns
/// Estimated (width, height) in pixels
pub fn estimate_tooltip_size(text: &str, max_width: f64, font_size: f64) -> (f64, f64) {
    let char_width = font_size * 0.6;
    let chars_per_line = (max_width / char_width).max(1.0) as usize;
    let line_count = (text.len() + chars_per_line - 1) / chars_per_line.max(1);
    let width = max_width.min(text.len() as f64 * char_width);
    let height = line_count as f64 * font_size * 1.5;
    (width.max(0.0), height.max(font_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_state_new() {
        let state = TooltipState::new();
        assert_eq!(state.show_delay_ms, 500.0);
        assert!(!state.visible);
        assert!(state.hovered_widget.is_none());
        assert_eq!(state.fade_in_duration_ms, 150.0);
        assert_eq!(state.fade_opacity, 0.0);
    }

    #[test]
    fn test_tooltip_state_with_delay() {
        let state = TooltipState::with_delay(1000.0);
        assert_eq!(state.show_delay_ms, 1000.0);
    }

    #[test]
    fn test_tooltip_state_with_config() {
        let config = TooltipConfig {
            show_delay_ms: 300.0,
            fade_in_duration_ms: 200.0,
            ..TooltipConfig::default()
        };
        let state = TooltipState::with_config(&config);
        assert_eq!(state.show_delay_ms, 300.0);
        assert_eq!(state.fade_in_duration_ms, 200.0);
    }

    #[test]
    fn test_set_delay() {
        let mut state = TooltipState::new();
        state.set_delay(750.0);
        assert_eq!(state.show_delay_ms, 750.0);
    }

    #[test]
    fn test_set_fade_duration() {
        let mut state = TooltipState::new();
        state.set_fade_duration(250.0);
        assert_eq!(state.fade_in_duration_ms, 250.0);
    }

    #[test]
    fn test_hover_tracking() {
        let mut state = TooltipState::new();
        let widget1 = WidgetId::new("button1");
        let widget2 = WidgetId::new("button2");

        // Start hovering widget1
        state.update(Some(widget1.clone()), 0.0);
        assert_eq!(state.hovered_widget(), Some(&widget1));
        assert_eq!(state.hover_start, 0.0);

        // Switch to widget2 - should reset timer
        state.update(Some(widget2.clone()), 300.0);
        assert_eq!(state.hovered_widget(), Some(&widget2));
        assert_eq!(state.hover_start, 300.0);
        assert!(!state.visible);

        // Stop hovering
        state.update(None, 400.0);
        assert!(state.hovered_widget().is_none());
        assert!(!state.visible);
    }

    #[test]
    fn test_tooltip_delay_timing() {
        let mut state = TooltipState::with_delay(500.0);
        let widget = WidgetId::new("button1");

        // Start hovering at time 0
        state.update(Some(widget.clone()), 0.0);

        // Before delay - should not show
        assert!(!state.should_show(400.0));

        // After delay - should show
        assert!(state.should_show(500.0));
        assert!(state.should_show(600.0));
    }

    #[test]
    fn test_tooltip_visibility() {
        let mut state = TooltipState::with_delay(500.0);
        let widget = WidgetId::new("button1");

        // Start hovering
        state.update(Some(widget.clone()), 0.0);
        assert!(!state.is_visible());

        // Request tooltip before delay
        state.request_tooltip(
            widget.clone(),
            "Test tooltip".to_string(),
            (100.0, 50.0),
            400.0,
        );
        assert!(!state.is_visible());
        assert!(state.get_active().is_none());

        // Request tooltip after delay
        state.request_tooltip(
            widget.clone(),
            "Test tooltip".to_string(),
            (100.0, 50.0),
            500.0,
        );
        assert!(state.is_visible());

        let tooltip = state.get_active().expect("Tooltip should be active");
        assert_eq!(tooltip.text, "Test tooltip");
        assert_eq!(tooltip.position, (100.0, 50.0));
    }

    #[test]
    fn test_tooltip_auto_visibility() {
        let mut state = TooltipState::with_delay(500.0);
        let widget = WidgetId::new("button1");

        // Start hovering and request tooltip
        state.update(Some(widget.clone()), 0.0);
        state.request_tooltip(
            widget.clone(),
            "Test".to_string(),
            (100.0, 50.0),
            0.0,
        );

        // Not visible yet
        assert!(!state.is_visible());

        // Update after delay - should become visible
        state.update(Some(widget.clone()), 500.0);
        assert!(state.is_visible());
    }

    #[test]
    fn test_tooltip_clear() {
        let mut state = TooltipState::with_delay(500.0);
        let widget = WidgetId::new("button1");

        state.update(Some(widget.clone()), 0.0);
        state.request_tooltip(
            widget.clone(),
            "Test".to_string(),
            (100.0, 50.0),
            500.0,
        );

        assert!(state.is_visible());

        state.clear();
        assert!(!state.is_visible());
        assert!(state.get_active().is_none());
        assert!(state.hovered_widget().is_none());
        assert_eq!(state.get_opacity(), 0.0);
    }

    #[test]
    fn test_tooltip_wrong_widget() {
        let mut state = TooltipState::with_delay(500.0);
        let widget1 = WidgetId::new("button1");
        let widget2 = WidgetId::new("button2");

        // Hover widget1
        state.update(Some(widget1.clone()), 0.0);

        // Request tooltip for widget2 - should be ignored
        state.request_tooltip(
            widget2.clone(),
            "Test".to_string(),
            (100.0, 50.0),
            500.0,
        );

        assert!(!state.is_visible());
        assert!(state.get_active().is_none());
    }

    #[test]
    fn test_calculate_position_basic() {
        let cursor = (100.0, 100.0);
        let tooltip_size = (150.0, 50.0);
        let screen_size = (1920.0, 1080.0);
        let offset = (10.0, 10.0);

        let pos = calculate_tooltip_position(cursor, tooltip_size, screen_size, offset);
        assert_eq!(pos, (110.0, 110.0));
    }

    #[test]
    fn test_calculate_position_right_edge() {
        let cursor = (1850.0, 100.0);
        let tooltip_size = (150.0, 50.0);
        let screen_size = (1920.0, 1080.0);
        let offset = (10.0, 10.0);

        let pos = calculate_tooltip_position(cursor, tooltip_size, screen_size, offset);
        // Should flip to left of cursor: 1850 - 150 - 10 = 1690
        assert_eq!(pos, (1690.0, 110.0));
    }

    #[test]
    fn test_calculate_position_bottom_edge() {
        let cursor = (100.0, 1050.0);
        let tooltip_size = (150.0, 50.0);
        let screen_size = (1920.0, 1080.0);
        let offset = (10.0, 10.0);

        let pos = calculate_tooltip_position(cursor, tooltip_size, screen_size, offset);
        // Should flip above cursor: 1050 - 50 - 10 = 990
        assert_eq!(pos, (110.0, 990.0));
    }

    #[test]
    fn test_calculate_position_corner() {
        let cursor = (1850.0, 1050.0);
        let tooltip_size = (150.0, 50.0);
        let screen_size = (1920.0, 1080.0);
        let offset = (10.0, 10.0);

        let pos = calculate_tooltip_position(cursor, tooltip_size, screen_size, offset);
        // Should flip both directions
        assert_eq!(pos, (1690.0, 990.0));
    }

    #[test]
    fn test_calculate_position_too_large() {
        let cursor = (50.0, 50.0);
        let tooltip_size = (2000.0, 1200.0);
        let screen_size = (1920.0, 1080.0);
        let offset = (10.0, 10.0);

        let pos = calculate_tooltip_position(cursor, tooltip_size, screen_size, offset);
        // Tooltip larger than screen - should clamp to edges
        assert_eq!(pos, (0.0, 0.0));
    }

    #[test]
    fn test_calculate_position_left_edge() {
        let cursor = (5.0, 100.0);
        let tooltip_size = (150.0, 50.0);
        let screen_size = (1920.0, 1080.0);
        let offset = (10.0, 10.0);

        let pos = calculate_tooltip_position(cursor, tooltip_size, screen_size, offset);
        // Would be at 15, which is fine (not clamped)
        assert_eq!(pos, (15.0, 110.0));
    }

    #[test]
    fn test_config_default() {
        let config = TooltipConfig::default();
        assert_eq!(config.show_delay_ms, 500.0);
        assert_eq!(config.offset, (10.0, 10.0));
        assert_eq!(config.max_width, 300.0);
        assert_eq!(config.fade_in_duration_ms, 150.0);
        assert_eq!(config.corner_radius, 4.0);
        assert_eq!(config.font_size, 12.0);
    }

    #[test]
    fn test_tooltip_request_creation() {
        let widget = WidgetId::new("button1");
        let request = TooltipRequest {
            text: "Click me!".to_string(),
            position: (100.0, 50.0),
            widget_id: widget.clone(),
            hover_start_time: 1000.0,
        };

        assert_eq!(request.text, "Click me!");
        assert_eq!(request.position, (100.0, 50.0));
        assert_eq!(request.widget_id, widget);
        assert_eq!(request.hover_start_time, 1000.0);
    }

    // --- New fade opacity tests ---

    #[test]
    fn test_fade_opacity_zero_before_visible() {
        let state = TooltipState::new();
        assert_eq!(state.get_opacity(), 0.0);
    }

    #[test]
    fn test_fade_opacity_linear_during_fade() {
        let mut state = TooltipState::with_delay(0.0);
        state.set_fade_duration(200.0);
        let widget = WidgetId::new("btn");

        // Tooltip becomes visible at time 0
        state.update(Some(widget.clone()), 0.0);
        assert!(state.is_visible());

        // At halfway through fade
        state.update(Some(widget.clone()), 100.0);
        let opacity = state.get_opacity();
        assert!((opacity - 0.5).abs() < 1e-9, "Expected 0.5, got {opacity}");
    }

    #[test]
    fn test_fade_opacity_full_after_fade_completes() {
        let mut state = TooltipState::with_delay(0.0);
        state.set_fade_duration(200.0);
        let widget = WidgetId::new("btn");

        state.update(Some(widget.clone()), 0.0);
        // Past the full fade duration
        state.update(Some(widget.clone()), 300.0);
        assert_eq!(state.get_opacity(), 1.0);
    }

    #[test]
    fn test_fade_opacity_resets_on_widget_change() {
        let mut state = TooltipState::with_delay(0.0);
        state.set_fade_duration(200.0);
        let widget1 = WidgetId::new("btn1");
        let widget2 = WidgetId::new("btn2");

        state.update(Some(widget1.clone()), 0.0);
        state.update(Some(widget1.clone()), 300.0);
        assert_eq!(state.get_opacity(), 1.0);

        // Switch widget — opacity should reset
        state.update(Some(widget2.clone()), 400.0);
        assert_eq!(state.get_opacity(), 0.0);
    }

    #[test]
    fn test_fade_opacity_zero_after_clear() {
        let mut state = TooltipState::with_delay(0.0);
        let widget = WidgetId::new("btn");
        state.update(Some(widget.clone()), 0.0);
        state.update(Some(widget.clone()), 300.0);
        assert_eq!(state.get_opacity(), 1.0);

        state.clear();
        assert_eq!(state.get_opacity(), 0.0);
    }

    // --- estimate_tooltip_size tests ---

    #[test]
    fn test_estimate_tooltip_size_short_text() {
        // "Hi" = 2 chars, font_size=12, char_width=7.2, max_width=300
        // width = min(300, 2*7.2) = 14.4; 2 chars fit in one line; height = 1 * 18 = 18
        let (w, h) = estimate_tooltip_size("Hi", 300.0, 12.0);
        assert!((w - 14.4).abs() < 1e-6, "width={w}");
        assert!((h - 18.0).abs() < 1e-6, "height={h}");
    }

    #[test]
    fn test_estimate_tooltip_size_respects_max_width() {
        let text = "A".repeat(200);
        let (w, _) = estimate_tooltip_size(&text, 100.0, 12.0);
        assert!(w <= 100.0, "width {w} should not exceed max_width=100");
    }

    #[test]
    fn test_estimate_tooltip_size_empty_text() {
        // Empty text: width=0 but clamped to 0, height clamped to font_size
        let (w, h) = estimate_tooltip_size("", 300.0, 12.0);
        assert_eq!(w, 0.0);
        assert_eq!(h, 12.0);
    }

    // --- TooltipTheme trait ---

    #[test]
    fn test_default_tooltip_theme_values() {
        let theme = DefaultTooltipTheme;
        assert_eq!(theme.background_color(), "#323232");
        assert_eq!(theme.text_color(), "#ffffff");
        assert_eq!(theme.border_color(), "#505050");
        assert_eq!(theme.border_width(), 1.0);
        assert_eq!(theme.corner_radius(), 4.0);
        assert_eq!(theme.padding(), 6.0);
        assert_eq!(theme.font_size(), 12.0);
        assert_eq!(theme.shadow_color(), "#00000060");
        assert_eq!(theme.shadow_blur(), 4.0);
        assert_eq!(theme.shadow_offset(), (0.0, 2.0));
    }
}
