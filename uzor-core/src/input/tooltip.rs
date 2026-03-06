//! Tooltip system for widget hover information
//!
//! Provides a centralized system for managing tooltips that appear after hovering
//! over widgets for a configurable delay period.

use super::widget_state::WidgetId;

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

/// Configuration for tooltip behavior
#[derive(Clone, Debug)]
pub struct TooltipConfig {
    /// Delay before showing (ms)
    pub show_delay_ms: f64,
    /// Offset from cursor
    pub offset: (f64, f64),
    /// Max width before wrapping
    pub max_width: f64,
}

impl Default for TooltipConfig {
    fn default() -> Self {
        Self {
            show_delay_ms: 500.0,
            offset: (10.0, 10.0),
            max_width: 300.0,
        }
    }
}

/// Manages tooltip display state
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
}

impl TooltipState {
    /// Create new tooltip state with default 500ms delay
    pub fn new() -> Self {
        Self {
            active: None,
            hovered_widget: None,
            hover_start: 0.0,
            show_delay_ms: 500.0,
            visible: false,
        }
    }

    /// Create new tooltip state with custom delay
    pub fn with_delay(delay_ms: f64) -> Self {
        Self {
            active: None,
            hovered_widget: None,
            hover_start: 0.0,
            show_delay_ms: delay_ms,
            visible: false,
        }
    }

    /// Set the tooltip delay
    pub fn set_delay(&mut self, delay_ms: f64) {
        self.show_delay_ms = delay_ms;
    }

    /// Update tooltip state based on currently hovered widget
    ///
    /// Call this each frame with the widget currently under the cursor.
    /// When the hovered widget changes, the hover timer resets.
    pub fn update(&mut self, hovered_widget: Option<WidgetId>, time: f64) {
        match (&self.hovered_widget, &hovered_widget) {
            // Widget changed - reset hover timer and hide tooltip
            (Some(old_id), Some(new_id)) if old_id != new_id => {
                self.hovered_widget = Some(new_id.clone());
                self.hover_start = time;
                self.visible = false;
                self.active = None;
            }
            // Started hovering a widget
            (None, Some(new_id)) => {
                self.hovered_widget = Some(new_id.clone());
                self.hover_start = time;
                self.visible = false;
            }
            // Stopped hovering
            (Some(_), None) => {
                self.hovered_widget = None;
                self.visible = false;
                self.active = None;
            }
            // Same widget or still no hover - keep current state
            _ => {}
        }

        // Update visibility based on delay
        if self.hovered_widget.is_some() && !self.visible {
            if self.should_show(time) {
                self.visible = true;
            }
        }
    }

    /// Request a tooltip for a specific widget
    ///
    /// This should be called by widgets that want to show a tooltip when hovered.
    /// The tooltip will only be shown if the widget has been hovered for the delay period.
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
                self.visible = self.should_show(time);
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
}

/// Calculate tooltip position near cursor, avoiding screen edges
///
/// Takes the cursor position, tooltip size, screen size, and desired offset,
/// and returns a position that keeps the tooltip on screen.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tooltip_state_new() {
        let state = TooltipState::new();
        assert_eq!(state.show_delay_ms, 500.0);
        assert!(!state.visible);
        assert!(state.hovered_widget.is_none());
    }

    #[test]
    fn test_tooltip_state_with_delay() {
        let state = TooltipState::with_delay(1000.0);
        assert_eq!(state.show_delay_ms, 1000.0);
    }

    #[test]
    fn test_set_delay() {
        let mut state = TooltipState::new();
        state.set_delay(750.0);
        assert_eq!(state.show_delay_ms, 750.0);
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
}
