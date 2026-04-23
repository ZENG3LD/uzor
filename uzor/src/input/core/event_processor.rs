//! Event processor - converts platform events to InputState updates
//!
//! This module bridges platform-specific events to uzor's platform-agnostic InputState.
//!
//! # Architecture
//!
//! ```text
//! Platform Events → EventProcessor → InputState → Widgets
//! ```
//!
//! The EventProcessor maintains transient state needed for multi-click detection
//! and gesture recognition, while InputState contains the per-frame snapshot
//! that widgets consume.

use crate::input::pointer::state::{InputState, MouseButton, ModifierKeys};
use crate::input::pointer::touch::TouchState;
use super::widget_state::WidgetInputState;
use crate::platform::PlatformEvent;

// =============================================================================
// EventProcessor
// =============================================================================

/// Processes platform events and updates InputState
///
/// This processor maintains state needed for gesture recognition and multi-click
/// detection. It converts low-level platform events into the high-level InputState
/// snapshot that widgets use for interaction detection.
///
/// # Example
///
/// ```ignore
/// let mut processor = EventProcessor::new();
/// let mut input = InputState::new();
///
/// // In event loop:
/// for event in platform_events {
///     processor.process(&event, &mut input, current_time);
/// }
///
/// // Now input is ready for widget rendering
/// ```
#[derive(Debug)]
pub struct EventProcessor {
    /// Current modifier key state
    modifiers: ModifierKeys,

    /// Whether we're in a touch interaction (affects click detection)
    touch_active: bool,

    /// Time of last event (for timing-based detection)
    last_event_time: f64,

    /// Active touch points (id -> position)
    active_touches: std::collections::HashMap<u64, (f64, f64)>,
}

impl Default for EventProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl EventProcessor {
    /// Create a new event processor
    pub fn new() -> Self {
        Self {
            modifiers: ModifierKeys::default(),
            touch_active: false,
            last_event_time: 0.0,
            active_touches: std::collections::HashMap::new(),
        }
    }

    /// Process a platform event and update InputState
    ///
    /// Returns true if the event was handled and may require a redraw.
    ///
    /// # Arguments
    ///
    /// * `event` - The platform event to process
    /// * `input` - The InputState to update
    /// * `time` - Current time in seconds (for multi-click detection)
    pub fn process(&mut self, event: &PlatformEvent, input: &mut InputState, time: f64) -> bool {
        self.last_event_time = time;

        match event {
            PlatformEvent::WindowCreated
            | PlatformEvent::WindowMoved { .. }
            | PlatformEvent::WindowDestroyed
            | PlatformEvent::ClipboardPaste { .. }
            | PlatformEvent::FileDropped { .. }
            | PlatformEvent::FileHovered { .. }
            | PlatformEvent::FileCancelled
            | PlatformEvent::Ime(_)
            | PlatformEvent::ThemeChanged { .. }
            | PlatformEvent::ScaleFactorChanged { .. } => false,

            // Pointer events
            PlatformEvent::PointerMoved { x, y } => {
                input.pointer.prev_pos = input.pointer.pos;
                input.pointer.pos = Some((*x, *y));
                true
            }

            PlatformEvent::PointerDown { x, y, button } => {
                input.pointer.pos = Some((*x, *y));
                input.pointer.button_down = Some(*button);
                true
            }

            PlatformEvent::PointerUp { x, y, button } => {
                input.pointer.pos = Some((*x, *y));
                if input.pointer.button_down == Some(*button) {
                    input.pointer.button_down = None;
                    // Mark as clicked this frame
                    input.pointer.clicked = Some(*button);
                }
                true
            }

            PlatformEvent::PointerEntered => {
                // Pointer entered window
                true
            }

            PlatformEvent::PointerLeft => {
                input.pointer.pos = None;
                input.pointer.button_down = None;
                true
            }

            // Touch events (basic single-touch compatibility)
            PlatformEvent::TouchStart { id, x, y } => {
                self.touch_active = true;
                self.active_touches.insert(*id, (*x, *y));

                // Initialize TouchState if needed
                if input.multi_touch.is_none() {
                    input.multi_touch = Some(TouchState::new());
                }

                // Update TouchState
                if let Some(ref mut touch) = input.multi_touch {
                    touch.update_touch(*id, *x, *y, time, None);
                }

                // For single-touch compatibility, update pointer
                if self.active_touches.len() == 1 {
                    input.pointer.pos = Some((*x, *y));
                    input.pointer.button_down = Some(MouseButton::Left);
                }
                true
            }

            PlatformEvent::TouchMove { id, x, y } => {
                if let Some(prev_pos) = self.active_touches.get_mut(id) {
                    *prev_pos = (*x, *y);
                }

                // Update TouchState
                if let Some(ref mut touch) = input.multi_touch {
                    touch.update_touch(*id, *x, *y, time, None);
                }

                // Update pointer for primary touch
                if self.active_touches.len() == 1 && self.active_touches.contains_key(id) {
                    input.pointer.prev_pos = input.pointer.pos;
                    input.pointer.pos = Some((*x, *y));
                }
                true
            }

            PlatformEvent::TouchEnd { id, x, y } => {
                let was_single_touch = self.active_touches.len() == 1;
                self.active_touches.remove(id);

                // Update TouchState
                if let Some(ref mut touch) = input.multi_touch {
                    touch.remove_touch(*id);
                }

                // Generate click for single touch
                if was_single_touch {
                    input.pointer.pos = Some((*x, *y));
                    input.pointer.button_down = None;
                    input.pointer.clicked = Some(MouseButton::Left);
                }

                if self.active_touches.is_empty() {
                    self.touch_active = false;
                }
                true
            }

            PlatformEvent::TouchCancel { id } => {
                self.active_touches.remove(id);

                // Update TouchState
                if let Some(ref mut touch) = input.multi_touch {
                    touch.remove_touch(*id);
                }

                if self.active_touches.is_empty() {
                    self.touch_active = false;
                    input.pointer.button_down = None;
                }
                true
            }

            // Scroll
            PlatformEvent::Scroll { dx, dy } => {
                input.scroll_delta = (*dx, *dy);
                true
            }

            // Keyboard
            PlatformEvent::KeyDown { modifiers, .. } => {
                self.modifiers = *modifiers;
                input.modifiers = *modifiers;
                // Key press handling would go to WidgetInputState
                true
            }

            PlatformEvent::KeyUp { modifiers, .. } => {
                self.modifiers = *modifiers;
                input.modifiers = *modifiers;
                true
            }

            PlatformEvent::ModifiersChanged { modifiers } => {
                self.modifiers = *modifiers;
                input.modifiers = *modifiers;
                false // No redraw needed for modifier change alone
            }

            PlatformEvent::TextInput { .. } => {
                // Text input would go to focused widget
                true
            }

            // Window events
            PlatformEvent::WindowResized { .. } => true,
            PlatformEvent::RedrawRequested => true,
            PlatformEvent::WindowFocused(focused) => {
                if !focused {
                    // Clear state when window loses focus
                    input.pointer.button_down = None;
                    input.modifiers = ModifierKeys::default();
                }
                true
            }

            // Other events
            PlatformEvent::WindowCloseRequested => false,
        }
    }

    /// Process event for widget state (double-click, drag detection)
    ///
    /// This updates the WidgetInputState which tracks per-widget interactions
    /// like hover, focus, and multi-click detection.
    pub fn process_widget(&mut self, event: &PlatformEvent, widget_state: &mut WidgetInputState, _time: f64) {
        match event {
            PlatformEvent::PointerMoved { x, y } => {
                widget_state.update_mouse(*x, *y);
            }
            PlatformEvent::PointerDown { .. } => {
                // Widget press is handled by hit testing in application
            }
            PlatformEvent::PointerUp { .. } => {
                // Widget release triggers click detection
                // Actual widget ID comes from hit testing
            }
            _ => {}
        }
    }

    /// Get current modifiers
    pub fn modifiers(&self) -> ModifierKeys {
        self.modifiers
    }

    /// Get number of active touches
    pub fn touch_count(&self) -> usize {
        self.active_touches.len()
    }

    /// Check if any touch is active
    pub fn has_active_touches(&self) -> bool {
        !self.active_touches.is_empty()
    }

    /// Reset processor state (e.g., when focus is lost)
    pub fn reset(&mut self) {
        self.modifiers = ModifierKeys::default();
        self.touch_active = false;
        self.active_touches.clear();
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pointer_move() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        processor.process(&PlatformEvent::PointerMoved { x: 100.0, y: 200.0 }, &mut input, 0.0);

        assert_eq!(input.pointer.pos, Some((100.0, 200.0)));
    }

    #[test]
    fn test_pointer_click() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        processor.process(&PlatformEvent::PointerDown { x: 100.0, y: 200.0, button: MouseButton::Left }, &mut input, 0.0);
        assert_eq!(input.pointer.button_down, Some(MouseButton::Left));

        processor.process(&PlatformEvent::PointerUp { x: 100.0, y: 200.0, button: MouseButton::Left }, &mut input, 0.1);
        assert_eq!(input.pointer.button_down, None);
        assert_eq!(input.pointer.clicked, Some(MouseButton::Left));
    }

    #[test]
    fn test_touch_to_pointer() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        processor.process(&PlatformEvent::TouchStart { id: 1, x: 100.0, y: 200.0 }, &mut input, 0.0);

        // Single touch should update pointer
        assert_eq!(input.pointer.pos, Some((100.0, 200.0)));
        assert_eq!(input.pointer.button_down, Some(MouseButton::Left));
        assert_eq!(processor.touch_count(), 1);

        // Should also update TouchState
        assert!(input.multi_touch.is_some());
        assert_eq!(input.touch_count(), 1);
    }

    #[test]
    fn test_multi_touch_tracking() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        // Two finger touch
        processor.process(&PlatformEvent::TouchStart { id: 1, x: 100.0, y: 200.0 }, &mut input, 0.0);
        processor.process(&PlatformEvent::TouchStart { id: 2, x: 200.0, y: 200.0 }, &mut input, 0.0);

        assert_eq!(processor.touch_count(), 2);
        assert!(processor.has_active_touches());

        // Check InputState also has touches
        assert_eq!(input.touch_count(), 2);
    }

    #[test]
    fn test_touch_end_generates_click() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        processor.process(&PlatformEvent::TouchStart { id: 1, x: 100.0, y: 200.0 }, &mut input, 0.0);
        processor.process(&PlatformEvent::TouchEnd { id: 1, x: 100.0, y: 200.0 }, &mut input, 0.1);

        assert_eq!(input.pointer.clicked, Some(MouseButton::Left));
        assert_eq!(processor.touch_count(), 0);
    }

    #[test]
    fn test_scroll() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        processor.process(&PlatformEvent::Scroll { dx: 0.0, dy: 10.0 }, &mut input, 0.0);

        assert_eq!(input.scroll_delta, (0.0, 10.0));
    }

    #[test]
    fn test_modifiers() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        let mods = ModifierKeys { ctrl: true, ..Default::default() };
        processor.process(&PlatformEvent::ModifiersChanged { modifiers: mods }, &mut input, 0.0);

        assert!(input.modifiers.ctrl);
        assert!(processor.modifiers().ctrl);
    }

    #[test]
    fn test_window_focus_clears_state() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        // Set some state
        input.pointer.button_down = Some(MouseButton::Left);
        input.modifiers.ctrl = true;

        // Lose focus
        processor.process(&PlatformEvent::WindowFocused(false), &mut input, 0.0);

        assert_eq!(input.pointer.button_down, None);
        assert!(!input.modifiers.ctrl);
    }

    #[test]
    fn test_pointer_left_clears_position() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        input.pointer.pos = Some((100.0, 200.0));
        processor.process(&PlatformEvent::PointerLeft, &mut input, 0.0);

        assert_eq!(input.pointer.pos, None);
    }

    #[test]
    fn test_touch_cancel() {
        let mut processor = EventProcessor::new();
        let mut input = InputState::new();

        processor.process(&PlatformEvent::TouchStart { id: 1, x: 100.0, y: 200.0 }, &mut input, 0.0);
        processor.process(&PlatformEvent::TouchCancel { id: 1 }, &mut input, 0.1);

        assert_eq!(processor.touch_count(), 0);
        assert!(!processor.has_active_touches());
    }

    #[test]
    fn test_reset() {
        let mut processor = EventProcessor::new();
        processor.modifiers.ctrl = true;
        processor.active_touches.insert(1, (100.0, 200.0));
        processor.touch_active = true;

        processor.reset();

        assert!(!processor.modifiers.ctrl);
        assert_eq!(processor.touch_count(), 0);
        assert!(!processor.touch_active);
    }
}
