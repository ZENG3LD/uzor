//! Platform-agnostic input state
//!
//! This module provides `InputState` - a snapshot of user input
//! that platforms populate and pass to rendering/widget code.

use crate::types::Rect;
use serde::{Deserialize, Serialize};

use super::touch::TouchState;

// =============================================================================
// MouseButton
// =============================================================================

/// Mouse button identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

// =============================================================================
// ModifierKeys
// =============================================================================

/// Keyboard modifier keys state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ModifierKeys {
    /// Shift key is held
    pub shift: bool,

    /// Ctrl key (or Cmd on Mac) is held
    pub ctrl: bool,

    /// Alt key (or Option on Mac) is held
    pub alt: bool,

    /// Meta key (Cmd on Mac, Win on Windows) is held
    pub meta: bool,
}

impl ModifierKeys {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn shift() -> Self {
        Self {
            shift: true,
            ..Default::default()
        }
    }

    pub fn ctrl() -> Self {
        Self {
            ctrl: true,
            ..Default::default()
        }
    }

    pub fn any(&self) -> bool {
        self.shift || self.ctrl || self.alt || self.meta
    }

    #[inline]
    pub fn ctrl_shift(&self) -> bool {
        self.ctrl && self.shift && !self.alt && !self.meta
    }

    #[inline]
    pub fn ctrl_alt(&self) -> bool {
        self.ctrl && self.alt && !self.shift && !self.meta
    }

    #[inline]
    pub fn command(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            self.meta
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.ctrl
        }
    }
}

// =============================================================================
// PointerState
// =============================================================================

/// Mouse/touch pointer state
#[derive(Clone, Debug, Default)]
pub struct PointerState {
    /// Current pointer position (None if not over canvas)
    pub pos: Option<(f64, f64)>,

    /// Which button is currently held down (if any)
    pub button_down: Option<MouseButton>,

    /// Which button was clicked this frame (single click)
    pub clicked: Option<MouseButton>,

    /// Which button was double-clicked this frame
    pub double_clicked: Option<MouseButton>,

    /// Which button was triple-clicked this frame
    pub triple_clicked: Option<MouseButton>,

    /// Previous pointer position (for calculating delta)
    pub prev_pos: Option<(f64, f64)>,
}

impl PointerState {
    pub fn delta(&self) -> (f64, f64) {
        match (self.pos, self.prev_pos) {
            (Some((x, y)), Some((px, py))) => (x - px, y - py),
            _ => (0.0, 0.0),
        }
    }

    pub fn is_present(&self) -> bool {
        self.pos.is_some()
    }
}

// =============================================================================
// DragState
// =============================================================================

/// Active drag operation state
#[derive(Clone, Debug)]
pub struct DragState {
    pub start: (f64, f64),
    pub current: (f64, f64),
    pub delta: (f64, f64),
    pub total_delta: (f64, f64),
    pub button: MouseButton,
    pub initial_value: f64,
}

impl DragState {
    pub fn new(start: (f64, f64), current: (f64, f64), button: MouseButton) -> Self {
        let total_delta = (current.0 - start.0, current.1 - start.1);
        Self {
            start,
            current,
            delta: (0.0, 0.0),
            total_delta,
            button,
            initial_value: 0.0,
        }
    }

    pub fn update(&mut self, x: f64, y: f64) {
        self.delta = (x - self.current.0, y - self.current.1);
        self.current = (x, y);
        self.total_delta = (self.current.0 - self.start.0, self.current.1 - self.start.1);
    }

    pub fn is_dragging(&self, _id: &crate::types::state::WidgetId) -> bool {
        // Simple dragging check for now
        true
    }

    pub fn delta_tuple(&self) -> (f64, f64) {
        self.delta
    }
}

// =============================================================================
// InputState - Main Input Snapshot
// =============================================================================

/// Platform-agnostic input state snapshot
#[derive(Clone, Debug, Default)]
pub struct InputState {
    pub pointer: PointerState,
    pub modifiers: ModifierKeys,
    pub scroll_delta: (f64, f64),
    pub drag: Option<DragState>,
    pub dt: f64,
    pub time: f64,
    pub multi_touch: Option<TouchState>,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pointer_pos(mut self, x: f64, y: f64) -> Self {
        self.pointer.pos = Some((x, y));
        self
    }

    pub fn pointer_pos(&self) -> Option<(f64, f64)> {
        self.pointer.pos
    }

    pub fn is_hovered(&self, rect: &Rect) -> bool {
        if let Some((px, py)) = self.pointer.pos {
            rect.contains(px, py)
        } else {
            false
        }
    }

    pub fn is_clicked(&self) -> bool {
        self.pointer.clicked == Some(MouseButton::Left)
    }

    pub fn is_double_clicked(&self) -> bool {
        self.pointer.double_clicked == Some(MouseButton::Left)
    }

    pub fn is_middle_clicked(&self) -> bool {
        self.pointer.clicked == Some(MouseButton::Middle)
    }

    pub fn is_right_clicked(&self) -> bool {
        self.pointer.clicked == Some(MouseButton::Right)
    }

    pub fn is_mouse_down(&self) -> bool {
        self.pointer.button_down == Some(MouseButton::Left)
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn drag_delta(&self) -> Option<(f64, f64)> {
        self.drag.as_ref().map(|d| d.delta)
    }

    pub fn shift(&self) -> bool {
        self.modifiers.shift
    }

    pub fn ctrl(&self) -> bool {
        self.modifiers.ctrl
    }

    pub fn alt(&self) -> bool {
        self.modifiers.alt
    }

    pub fn consume_click(&mut self) -> bool {
        if self.pointer.clicked.is_some() {
            self.pointer.clicked = None;
            true
        } else {
            false
        }
    }

    pub fn consume_scroll(&mut self) -> (f64, f64) {
        let delta = self.scroll_delta;
        self.scroll_delta = (0.0, 0.0);
        delta
    }

    /// Get active touch count
    pub fn touch_count(&self) -> usize {
        self.multi_touch
            .as_ref()
            .map(|t| t.touch_count())
            .unwrap_or(0)
    }

    pub fn primary_pointer(&self) -> Option<(f64, f64)> {
        self.pointer.pos.or_else(|| {
            self.multi_touch
                .as_ref()
                .and_then(|t| t.primary_touch())
                .map(|t| t.pos)
        })
    }

    pub fn end_frame(&mut self) {
        self.pointer.clicked = None;
        self.pointer.double_clicked = None;
        self.pointer.triple_clicked = None;
        self.pointer.prev_pos = self.pointer.pos;
        if let Some(ref mut touch) = self.multi_touch {
            touch.clear_deltas();
        }
    }
}