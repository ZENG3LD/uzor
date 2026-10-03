//! [`CoordinatorDriver`] — the only door that mutates focus, hover, capture,
//! layer stack and text editing on an [`InputCoordinator`](super::InputCoordinator).
//!
//! Registration (`register`, `register_on_layer`, `register_text_field`, …)
//! and read methods stay inherent. Calling anything on this trait requires
//! `use uzor::input::driver::CoordinatorDriver`, which ban F7 allows only in
//! the framework input engine, the kernel, this module's tests, and
//! `uzor::testing`.

use super::{InputState, KeyPress, LayerId, TextAction, WidgetResponse};
use crate::types::WidgetId;

/// State-mutating half of [`InputCoordinator`](super::InputCoordinator).
///
/// One writer of focus, hover and capture: the framework's `InputEngine`
/// (and the kernel's compose path, which pushes overlay layers). App hook
/// contexts keep `&mut InputCoordinator` for registration only.
pub trait CoordinatorDriver {
    /// Start a frame: bake hover from the finished previous frame, clear
    /// registrations and layers, install the main layer, and propagate
    /// `input` into scoped regions.
    fn begin_frame(&mut self, input: InputState);

    /// Hit-test this frame's registrations and return the widgets that
    /// interacted. Scoped-region responses are prefixed and come first.
    fn end_frame(&mut self) -> Vec<(WidgetId, WidgetResponse)>;

    /// Update the cursor position without resetting the rest of the frame.
    fn set_cursor_pos(&mut self, x: f64, y: f64);

    /// Top-most widget with `sense.click` at `(x, y)`.
    fn process_click(&self, x: f64, y: f64) -> Option<WidgetId>;

    /// Top-most widget with `sense.right_click` at `(x, y)`.
    fn process_right_click(&self, x: f64, y: f64) -> Option<WidgetId>;

    /// Top-most widget with `sense.double_click` at `(x, y)`.
    fn process_double_click(&self, x: f64, y: f64) -> Option<WidgetId>;

    /// Top-most widget with `sense.scroll` at `(x, y)`.
    fn process_scroll(&self, x: f64, y: f64) -> Option<WidgetId>;

    /// Top-most widget with `sense.drag` at `(x, y)`, without starting a drag.
    fn process_drag_press(&self, x: f64, y: f64) -> Option<WidgetId>;

    /// Top-most widget with `sense.hover` at `(x, y)`.
    fn process_hover(&self, x: f64, y: f64) -> Option<WidgetId>;

    /// Begin a drag on the top-most `sense.drag` widget at `(x, y)`.
    fn process_drag_start(&mut self, x: f64, y: f64) -> Option<WidgetId>;

    /// Update an in-progress drag. `None` when no drag is active.
    fn process_drag_move(&mut self, x: f64, y: f64) -> Option<(WidgetId, f64, f64)>;

    /// End an in-progress drag. `None` when no drag is active.
    fn process_drag_end(&mut self) -> Option<WidgetId>;

    /// Focus `id`. A non-text target blurs any focused text field.
    fn set_focus(&mut self, id: impl Into<WidgetId>);

    /// Clear widget focus and blur the text store.
    fn clear_focus(&mut self);

    /// Focus the next focusable widget registered this frame (Tab).
    fn focus_next(&mut self);

    /// Focus the previous focusable widget registered this frame (Shift+Tab).
    fn focus_prev(&mut self);

    /// Route pointer input to `id` until [`release_pointer`](Self::release_pointer)
    /// or the current press ends.
    fn grab_pointer(&mut self, id: impl Into<WidgetId>);

    /// End a pointer grab early.
    fn release_pointer(&mut self);

    /// Forward a printable character to the focused text field.
    fn on_char(&mut self, ch: char) -> TextAction;

    /// Forward a named key to the focused text field.
    fn on_key(&mut self, key: KeyPress) -> TextAction;

    /// Push a layer. Chrome defaults to `false`; use the inherent
    /// `push_layer_ex` when a layer must stay hit-testable under a modal.
    fn push_layer(&mut self, id: LayerId, z_order: u32, modal: bool);

    /// Pop a layer. No-op: layers live until the next `begin_frame`.
    fn pop_layer(&mut self, id: &LayerId);
}
