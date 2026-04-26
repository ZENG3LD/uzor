//! Tooltip state — hover timing and fade-in progress.

use super::types::TooltipConfig;

/// Per-tooltip persistent state managed by the caller.
#[derive(Debug, Clone)]
pub struct TooltipState {
    /// Currently active tooltip configuration, if any.
    pub active: Option<TooltipConfig>,
    /// Timestamp (ms) when the hovered widget was first detected.
    pub hover_start_ms: f64,
    /// Delay (ms) before the tooltip becomes visible.
    pub show_delay_ms: f64,
    /// Fade-in progress: 0.0 = fully transparent, 1.0 = fully opaque.
    pub fade_in_progress: f32,
}

impl Default for TooltipState {
    fn default() -> Self {
        Self {
            active: None,
            hover_start_ms: 0.0,
            show_delay_ms: 500.0,
            fade_in_progress: 0.0,
        }
    }
}

impl TooltipState {
    /// Create a new state with a custom show delay.
    pub fn with_delay(show_delay_ms: f64) -> Self {
        Self { show_delay_ms, ..Self::default() }
    }

    /// Call when the pointer enters a widget that should show a tooltip.
    ///
    /// Supply `now_ms` as the current wall-clock time in milliseconds.
    pub fn set_hover(&mut self, config: TooltipConfig, now_ms: f64) {
        if self.active.is_none() {
            self.hover_start_ms = now_ms;
            self.fade_in_progress = 0.0;
        }
        self.active = Some(config);
    }

    /// Call when the pointer leaves the widget.
    pub fn clear_hover(&mut self) {
        self.active = None;
        self.fade_in_progress = 0.0;
        self.hover_start_ms = 0.0;
    }

    /// Advance the fade-in animation.
    ///
    /// `now_ms` — current time in milliseconds.
    /// `fade_duration_ms` — how long the fade takes once the delay has elapsed.
    pub fn tick(&mut self, now_ms: f64, fade_duration_ms: f64) {
        if self.active.is_none() {
            self.fade_in_progress = 0.0;
            return;
        }
        let elapsed = now_ms - self.hover_start_ms - self.show_delay_ms;
        if elapsed <= 0.0 {
            self.fade_in_progress = 0.0;
        } else if fade_duration_ms > 0.0 {
            self.fade_in_progress = ((elapsed / fade_duration_ms) as f32).clamp(0.0, 1.0);
        } else {
            self.fade_in_progress = 1.0;
        }
    }

    /// Returns `true` when the tooltip should be rendered (delay elapsed).
    pub fn should_show(&self) -> bool {
        self.active.is_some() && self.fade_in_progress > 0.0
    }
}
