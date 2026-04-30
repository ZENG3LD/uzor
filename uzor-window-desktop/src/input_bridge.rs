//! `WinitInputBridge` — reusable winit→InputCoordinator handler.
//!
//! Extracts the boilerplate of routing raw winit events into uzor's
//! input system: pointer position tracking, click detection, drag
//! start/end, mouse wheel deltas, keyboard with text-field routing
//! (including Ctrl+A/C/V/X), and modifier state.
//!
//! Apps still own their own widget hover/press state machine and
//! per-widget drag logic; this bridge handles only the event-to-coord
//! plumbing that's identical across all desktop apps.

use winit::event::{ElementState, MouseScrollDelta, WindowEvent, MouseButton as WMouseButton};
use winit::keyboard::{Key, NamedKey};

use uzor::input::core::coordinator::InputCoordinator;
use uzor::input::keyboard::keyboard::KeyPress;
use uzor::types::WidgetId;

/// Keyboard modifier state, updated on each `ModifiersChanged` event.
#[derive(Default, Debug, Clone, Copy)]
pub struct ModifierState {
    pub shift: bool,
    pub ctrl:  bool,
    pub alt:   bool,
    pub meta:  bool,
}

/// Output of one `handle_event` call. App reads this to drive its own state machine.
#[derive(Default, Debug)]
pub struct BridgeOutput {
    /// Cursor moved to this position (logical pixels, window-relative).
    pub cursor_moved: Option<(f64, f64)>,
    /// Left mouse pressed at this position.
    pub left_down: Option<(f64, f64)>,
    /// Left mouse released at this position.  `clicked_id` is the top-most
    /// widget with `sense.click` at that point (from `process_click`).
    pub left_up: Option<((f64, f64), Option<WidgetId>)>,
    /// Right mouse released at this position.
    pub right_up: Option<(f64, f64)>,
    /// Mouse wheel delta at the current cursor position (logical lines).
    /// Inner tuple: `((cursor_x, cursor_y), (dx, dy))`.
    pub wheel: Option<((f64, f64), (f64, f64))>,
    /// `true` if a key event modified text-field state this frame.
    pub text_changed: bool,
    /// `true` if focus was cleared this frame (Enter or Escape on focused field).
    pub focus_cleared: bool,
}

/// Translates raw winit `WindowEvent`s into `InputCoordinator` calls.
///
/// Owns the arboard clipboard handle (lazily initialised on first use).
/// Text-field drag tracking is internal: the bridge keeps track of which
/// field is being drag-selected and calls `on_drag_start/move/end` on the
/// `TextFieldStore` transparently.
pub struct WinitInputBridge {
    /// Current keyboard modifier state.
    pub modifiers: ModifierState,
    /// Last known pointer position in logical pixels.
    pub last_mouse_pos: (f64, f64),
    /// `arboard` clipboard, lazily constructed.
    clipboard: Option<arboard::Clipboard>,
    /// Whether a text-field drag-selection is in progress (LMB held inside a
    /// text field).  The `TextFieldStore` internally tracks which field owns the
    /// drag; we only need a boolean here to know when to call `on_drag_end`.
    text_dragging: bool,
}

impl WinitInputBridge {
    /// Create a new bridge with default state.
    pub fn new() -> Self {
        Self {
            modifiers: ModifierState::default(),
            last_mouse_pos: (0.0, 0.0),
            clipboard: None,
            text_dragging: false,
        }
    }

    /// Get or lazily initialise the clipboard handle.
    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    /// Process one winit `WindowEvent`.
    ///
    /// Returns a `BridgeOutput` describing what happened this event. The app
    /// should use the output to update its own state and request redraws.
    ///
    /// `focused_text_field` — the `WidgetId` of the currently focused text field
    /// (from `coord.focused_widget()` or app-tracked focus), or `None` if no text
    /// field is focused.  Keyboard events are forwarded to the text-field store
    /// only when this is `Some`.
    pub fn handle_event(
        &mut self,
        coord: &mut InputCoordinator,
        focused_text_field: Option<&WidgetId>,
        event: &WindowEvent,
    ) -> BridgeOutput {
        let mut out = BridgeOutput::default();

        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let pos = (position.x, position.y);
                self.last_mouse_pos = pos;
                out.cursor_moved = Some(pos);

                // Extend text-field drag selection while LMB is held.
                if self.text_dragging {
                    coord.text_fields_mut().on_drag_move(pos.0);
                    out.text_changed = true;
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: WMouseButton::Left,
                ..
            } => {
                let (x, y) = self.last_mouse_pos;
                out.left_down = Some((x, y));

                // Start text-field drag selection.  TextFieldStore::on_drag_start
                // does its own hit-test using stored rects and only activates if
                // the point lands on a registered field.
                coord.text_fields_mut().on_drag_start(x, y);
                // Track whether a drag was actually started.
                self.text_dragging = coord.text_fields().focused().is_some();
                if self.text_dragging {
                    out.text_changed = true;
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: WMouseButton::Left,
                ..
            } => {
                let (x, y) = self.last_mouse_pos;

                // End text-field drag selection.
                if self.text_dragging {
                    coord.text_fields_mut().on_drag_end();
                    self.text_dragging = false;
                    out.text_changed = true;
                }

                let clicked = coord.process_click(x, y);
                out.left_up = Some(((x, y), clicked));
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: WMouseButton::Right,
                ..
            } => {
                let (x, y) = self.last_mouse_pos;
                out.right_up = Some((x, y));
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x as f64, *y as f64),
                    MouseScrollDelta::PixelDelta(p)   => (p.x / 20.0, p.y / 20.0),
                };
                out.wheel = Some((self.last_mouse_pos, (dx, dy)));
            }

            WindowEvent::ModifiersChanged(m) => {
                let st = m.state();
                self.modifiers.shift = st.shift_key();
                self.modifiers.ctrl  = st.control_key();
                self.modifiers.alt   = st.alt_key();
                self.modifiers.meta  = st.super_key();
            }

            WindowEvent::KeyboardInput { event: ke, .. }
                if ke.state == ElementState::Pressed =>
            {
                if let Some(id) = focused_text_field {
                    let consumed = self.handle_text_key(coord, id, &ke.logical_key);
                    if consumed {
                        out.text_changed = true;
                    } else if let Key::Named(NamedKey::Escape) = ke.logical_key {
                        coord.clear_focus();
                        out.focus_cleared = true;
                    } else if let Key::Named(NamedKey::Enter) = ke.logical_key {
                        coord.clear_focus();
                        out.focus_cleared = true;
                    }
                }
            }

            _ => {}
        }

        out
    }

    /// Route a key press to the focused text field.  Returns `true` if the key
    /// was consumed by the text-field store (i.e. a text-editing key).
    fn handle_text_key(
        &mut self,
        coord: &mut InputCoordinator,
        _id: &WidgetId,
        key: &Key,
    ) -> bool {
        let m = self.modifiers;

        // ── Ctrl shortcuts ────────────────────────────────────────────────────
        if m.ctrl {
            if let Key::Character(s) = key {
                let ch = s.chars().next().map(|c| c.to_ascii_lowercase());
                match ch {
                    Some('a') => {
                        coord.text_fields_mut().on_key(KeyPress::SelectAll);
                        return true;
                    }
                    Some('c') => {
                        // Copy selection to clipboard.
                        if let Some(sel) = coord.text_fields().copy_selection() {
                            if let Some(cb) = self.clipboard() {
                                let _ = cb.set_text(sel);
                            }
                        }
                        return true;
                    }
                    Some('v') => {
                        let text = self
                            .clipboard()
                            .and_then(|cb| cb.get_text().ok())
                            .unwrap_or_default();
                        if !text.is_empty() {
                            coord.text_fields_mut().on_key(KeyPress::Paste(text));
                        }
                        return true;
                    }
                    Some('x') => {
                        // Cut: copy then delete selection.
                        if let Some(sel) = coord.text_fields().copy_selection() {
                            if let Some(cb) = self.clipboard() {
                                let _ = cb.set_text(sel);
                            }
                            coord.text_fields_mut().on_key(KeyPress::Delete);
                        }
                        return true;
                    }
                    _ => {}
                }
            }
        }

        // ── Named keys ────────────────────────────────────────────────────────
        match key {
            Key::Named(NamedKey::Backspace) => {
                coord.text_fields_mut().on_char('\x08');
                true
            }
            Key::Named(NamedKey::Delete) => {
                coord.text_fields_mut().on_key(KeyPress::Delete);
                true
            }
            Key::Named(NamedKey::Enter) => {
                // Enter commits / blurs — caller sees focus_cleared via outer branch.
                false
            }
            Key::Named(NamedKey::Escape) => {
                // Escape handled by outer branch.
                false
            }
            Key::Named(NamedKey::ArrowLeft) => {
                let kp = if m.shift { KeyPress::ShiftLeft } else { KeyPress::ArrowLeft };
                coord.text_fields_mut().on_key(kp);
                true
            }
            Key::Named(NamedKey::ArrowRight) => {
                let kp = if m.shift { KeyPress::ShiftRight } else { KeyPress::ArrowRight };
                coord.text_fields_mut().on_key(kp);
                true
            }
            Key::Named(NamedKey::Home) => {
                let kp = if m.shift { KeyPress::ShiftHome } else { KeyPress::Home };
                coord.text_fields_mut().on_key(kp);
                true
            }
            Key::Named(NamedKey::End) => {
                let kp = if m.shift { KeyPress::ShiftEnd } else { KeyPress::End };
                coord.text_fields_mut().on_key(kp);
                true
            }
            // Printable characters — forwarded through on_char.
            Key::Character(s) => {
                // Guard: skip if already handled above (ctrl combinations).
                if m.ctrl || m.meta {
                    return false;
                }
                let mut consumed = false;
                for ch in s.chars() {
                    if !ch.is_control() {
                        coord.text_fields_mut().on_char(ch);
                        consumed = true;
                    }
                }
                consumed
            }
            _ => false,
        }
    }

    /// Copy the current text-field selection to the clipboard.
    ///
    /// Convenience helper for callers that want to trigger copy outside of a
    /// keyboard event (e.g. a "Copy" toolbar button).
    pub fn copy_selection(&mut self, coord: &mut InputCoordinator) {
        if let Some(sel) = coord.text_fields().copy_selection() {
            if let Some(cb) = self.clipboard() {
                let _ = cb.set_text(sel);
            }
        }
    }

    /// Paste clipboard text into the focused text field.
    ///
    /// Convenience helper for callers that want to trigger paste outside of a
    /// keyboard event (e.g. a "Paste" toolbar button).
    pub fn paste(&mut self, coord: &mut InputCoordinator) {
        let text = self
            .clipboard()
            .and_then(|cb| cb.get_text().ok())
            .unwrap_or_default();
        if !text.is_empty() {
            coord.text_fields_mut().on_key(KeyPress::Paste(text));
        }
    }
}

impl Default for WinitInputBridge {
    fn default() -> Self {
        Self::new()
    }
}
