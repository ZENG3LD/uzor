//! Text field store — owns text/cursor/selection state for all registered fields.

use std::collections::HashMap;

use crate::types::WidgetId;
use crate::input::keyboard::keyboard::KeyPress;

// =============================================================================
// InputCapability
// =============================================================================

/// Which input sources a field accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputCapability {
    /// Accepts both keyboard and mouse input.
    Both,
    /// Accepts keyboard input only (ignores mouse drag/click).
    Keyboard,
    /// Accepts mouse input only (ignores char/key events).
    Mouse,
    /// Raw PTY mode — bytes forwarded directly without text editing logic.
    Raw,
}

// =============================================================================
// TextFieldConfig
// =============================================================================

/// Configuration for a single text field.
#[derive(Clone)]
pub struct TextFieldConfig {
    /// Which input sources this field accepts.
    pub capability: InputCapability,
    /// Whether this field is read-only (accepts SelectAll/Copy but not editing).
    pub read_only: bool,
    /// Optional character filter — returns `true` if the character is allowed.
    pub char_filter: Option<fn(char) -> bool>,
    /// Maximum number of characters allowed (None = unlimited).
    pub max_len: Option<usize>,
    /// Whether to mask input display (password field).
    /// The store still holds the real text — masking is a renderer concern.
    pub masked: bool,
}

impl TextFieldConfig {
    /// Plain editable text field.
    pub fn text() -> Self {
        Self {
            capability: InputCapability::Both,
            read_only: false,
            char_filter: None,
            max_len: None,
            masked: false,
        }
    }

    /// Password field (editable, both input sources, no display filter needed here).
    pub fn password() -> Self {
        Self {
            capability: InputCapability::Both,
            read_only: false,
            char_filter: None,
            max_len: None,
            masked: true,
        }
    }

    /// Search field (same as text for now).
    pub fn search() -> Self {
        Self::text()
    }

    /// Read-only display field (Copy and SelectAll still work).
    pub fn read_only() -> Self {
        Self {
            capability: InputCapability::Both,
            read_only: true,
            char_filter: None,
            max_len: None,
            masked: false,
        }
    }

    /// Keyboard-only field (ignores mouse drag/click).
    pub fn keyboard_only() -> Self {
        Self {
            capability: InputCapability::Keyboard,
            read_only: false,
            char_filter: None,
            max_len: None,
            masked: false,
        }
    }

    /// Raw PTY field — bytes forwarded directly.
    pub fn raw() -> Self {
        Self {
            capability: InputCapability::Raw,
            read_only: false,
            char_filter: None,
            max_len: None,
            masked: false,
        }
    }

    /// Builder: set a character filter.
    pub fn with_filter(mut self, filter: fn(char) -> bool) -> Self {
        self.char_filter = Some(filter);
        self
    }

    /// Builder: set a maximum character length.
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    /// Builder: set masked mode (for password display).
    pub fn with_masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }
}

// =============================================================================
// TextAction
// =============================================================================

/// Result of processing a character or key press.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAction {
    /// No action taken.
    None,
    /// User pressed Enter — contains the committed text.
    Commit(String),
    /// User pressed Escape — text reverted to `original_text`.
    Cancel,
    /// Text was modified — contains the new text.
    TextChanged(String),
    /// Raw PTY bytes (for Raw-capability fields).
    RawInput(Vec<u8>),
}

// =============================================================================
// TextFieldState
// =============================================================================

/// Mutable state for a single text field.
pub struct TextFieldState {
    /// Current text content.
    pub text: String,
    /// Snapshot of text at `begin_edit` time, used to revert on Cancel.
    pub original_text: String,
    /// Cursor position in char units (not bytes).
    pub cursor: usize,
    /// Selection anchor in char units. When `Some(anchor)`, the selection
    /// spans `min(anchor, cursor)..max(anchor, cursor)`.
    pub selection_start: Option<usize>,
    /// Last rendered bounding rect `(x, y, width, height)`.
    pub last_rect: Option<(f64, f64, f64, f64)>,
    /// X positions of char boundaries (length = char_count + 1).
    pub last_char_positions: Vec<f64>,
    /// Frame counter at last `update_field` call.
    pub last_frame: u64,
    /// Whether the user has TOUCHED this field — clicked into it or typed.
    ///
    /// Distinct from focus on purpose. A dialog arms its field on open so the
    /// first keystroke lands without a click, but arming is not an invitation:
    /// a caret blinking in a box nobody asked for reads as noise. Focus says
    /// "keystrokes come here"; this says "the user is in it", and only the
    /// second one should be visible.
    pub engaged: bool,
    /// Field configuration (immutable after registration).
    pub config: TextFieldConfig,
}

impl TextFieldState {
    /// Create a new state for the given config.
    pub fn new(config: TextFieldConfig) -> Self {
        Self {
            text: String::new(),
            original_text: String::new(),
            cursor: 0,
            selection_start: None,
            last_rect: None,
            last_char_positions: Vec::new(),
            last_frame: 0,
            engaged: false,
            config,
        }
    }

    /// Return `(lo, hi)` if there is a non-empty selection, else `None`.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.selection_start?;
        if anchor == self.cursor {
            return None;
        }
        let lo = anchor.min(self.cursor);
        let hi = anchor.max(self.cursor);
        Some((lo, hi))
    }

    /// Delete the current selection and position the cursor at `lo`.
    /// No-op if there is no selection.
    pub fn delete_selection(&mut self) {
        let (lo, hi) = match self.selection_range() {
            Some(range) => range,
            None => return,
        };
        let byte_lo = self.char_to_byte(lo);
        let byte_hi = self.char_to_byte(hi);
        self.text.drain(byte_lo..byte_hi);
        self.cursor = lo;
        self.selection_start = None;
    }

    /// Convert a char index to a byte index in `self.text`.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.text.len())
    }

    /// Return the number of chars in `self.text`.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }
}

// =============================================================================
// TextFieldStore
// =============================================================================

/// Central store that owns text/cursor/selection for all registered fields.
///
/// # Focus model
/// Only one field is focused at a time. Focusing a new field clears the
/// selection anchor on the previously focused field but does NOT clear its
/// text.
pub struct TextFieldStore {
    fields: HashMap<WidgetId, TextFieldState>,
    focused: Option<WidgetId>,
    drag_field: Option<WidgetId>,
    current_frame: u64,
    blink_reset_time: u64,
}

impl TextFieldStore {
    /// Frame-staleness tolerance for [`Self::focused_is_stale`], in
    /// frame-counter ticks. A host's `prepare_frame`/`render_to_scene` pair
    /// can advance the frame counter twice per real rendered frame (calling
    /// `begin_frame()` once each, so their own `update_field` calls land on
    /// the second of the two) — this tolerates one full extra tick of lag
    /// on top of that before a non-updating field counts as gone, so an
    /// ordinary render-to-render gap is never mistaken for staleness.
    const FOCUS_STALE_FRAME_LAG: u64 = 3;

    // =========================================================================
    // Lifecycle
    // =========================================================================

    /// Create an empty store.
    pub fn new() -> Self {
        Self {
            fields: HashMap::new(),
            focused: None,
            drag_field: None,
            current_frame: 0,
            blink_reset_time: 0,
        }
    }

    /// Advance the frame counter. Call at the start of every render frame.
    pub fn begin_frame(&mut self) {
        self.current_frame = self.current_frame.wrapping_add(1);
    }

    // =========================================================================
    // Registration
    // =========================================================================

    /// Register a field. If the field is already registered its config is
    /// updated but existing text/cursor state is preserved.
    pub fn register(&mut self, id: impl Into<WidgetId>, config: TextFieldConfig) {
        let id = id.into();
        let state = self.fields
            .entry(id)
            .or_insert_with(|| TextFieldState::new(config.clone()));
        state.config = config;
    }

    /// Remove a field. Clears focus/drag if it was pointing at this field.
    pub fn unregister(&mut self, id: &WidgetId) {
        self.fields.remove(id);
        if self.focused.as_ref() == Some(id) {
            self.focused = None;
        }
        if self.drag_field.as_ref() == Some(id) {
            self.drag_field = None;
        }
    }

    /// Refresh the screen geometry for a field. Call after rendering the field.
    ///
    /// `char_positions` is the list of char boundary X positions (length = char_count + 1).
    pub fn update_field(
        &mut self,
        id: &WidgetId,
        rect: (f64, f64, f64, f64),
        char_positions: Vec<f64>,
    ) {
        if let Some(state) = self.fields.get_mut(id) {
            state.last_rect = Some(rect);
            state.last_char_positions = char_positions;
            state.last_frame = self.current_frame;
        }
    }

    /// Stamp a field as "on screen this frame" WITHOUT touching its
    /// click-to-cursor geometry — the light-weight sibling of
    /// [`Self::update_field`] for a field whose on-screen model does not
    /// fit a real rect + char-boundary list (a whole-button pseudo-field
    /// with no per-char caret, a raw terminal grid with its own selection
    /// state, ...). Sets only the frame-liveness signal
    /// [`Self::focused_is_stale`] and [`Self::focused_was_never_stamped`]
    /// read; leaves `last_rect`/`last_char_positions` untouched, so calling
    /// this can never make [`Self::on_drag_start`]'s hit-test start
    /// matching a field whose host never drew a real, clickable text box
    /// for it. Every field that can hold focus must be stamped by EITHER
    /// this or `update_field` each frame its host draws it — one of the two
    /// is the whole contract `focused_is_stale` relies on.
    pub fn mark_seen(&mut self, id: &WidgetId) {
        if let Some(state) = self.fields.get_mut(id) {
            state.last_frame = self.current_frame;
        }
    }

    // =========================================================================
    // Query
    // =========================================================================

    /// Current text of a field.
    pub fn text(&self, id: &WidgetId) -> &str {
        self.fields.get(id).map(|s| s.text.as_str()).unwrap_or("")
    }

    /// Cursor position (in chars) of a field.
    pub fn cursor(&self, id: &WidgetId) -> usize {
        self.fields.get(id).map(|s| s.cursor).unwrap_or(0)
    }

    /// Selection range `(lo, hi)` if a non-empty selection exists.
    pub fn selection_range(&self, id: &WidgetId) -> Option<(usize, usize)> {
        self.fields.get(id)?.selection_range()
    }

    /// Whether the given field is currently focused.
    /// Whether the user has touched this field — see [`TextFieldState::engaged`].
    /// Draw the caret and the focus ring from THIS, not from focus.
    pub fn is_engaged(&self, id: &WidgetId) -> bool {
        self.fields.get(id).map(|s| s.engaged).unwrap_or(false)
    }

    /// Mark a field as touched, for a host that routes its own clicks.
    pub fn engage(&mut self, id: &WidgetId) {
        if let Some(state) = self.fields.get_mut(id) {
            state.engaged = true;
        }
    }

    pub fn is_focused(&self, id: &WidgetId) -> bool {
        self.focused.as_ref() == Some(id)
    }

    /// Currently focused field id.
    pub fn focused(&self) -> Option<&WidgetId> {
        self.focused.as_ref()
    }

    /// Whether the currently focused field's screen geometry has gone
    /// stale: it has not been stamped (`update_field` or [`Self::mark_seen`])
    /// for more than [`Self::FOCUS_STALE_FRAME_LAG`] frame-counter ticks.
    /// This is the shape a modal or inline editor leaves behind when it
    /// stops being drawn without first calling [`Self::blur`] on its own
    /// field — a caller can use this to detect and clear that leftover
    /// focus instead of trusting it forever.
    ///
    /// A field that has NEVER once been stamped (`last_frame == 0` since
    /// registration) is treated the same as one whose stamps stopped: it
    /// goes stale as soon as more than `FOCUS_STALE_FRAME_LAG` frames have
    /// elapsed since it was focused. Every field that can hold focus is
    /// expected to be stamped by its host on every frame it draws it — a
    /// caller that focuses a field and never stamps it has a real bug, and
    /// [`Self::focused_was_never_stamped`] is the sharper, immediate signal
    /// for that specific case (a caller can `debug_assert!` on it instead
    /// of waiting out the frame lag). `false` (not stale) is always the
    /// safe default when nothing is focused.
    pub fn focused_is_stale(&self) -> bool {
        let Some(id) = self.focused.as_ref() else { return false };
        let Some(state) = self.fields.get(id) else { return false };
        self.current_frame.wrapping_sub(state.last_frame) > Self::FOCUS_STALE_FRAME_LAG
    }

    /// Whether the currently focused field has NEVER once been stamped via
    /// `update_field` or [`Self::mark_seen`] — a regression tripwire.
    ///
    /// `focused_is_stale` alone eventually self-heals this shape (see its
    /// own doc), but only after `FOCUS_STALE_FRAME_LAG` frames of silently
    /// swallowed keystrokes. A caller that dispatches keys should check
    /// this FIRST and `debug_assert!`/`log::warn!` under
    /// `cfg(debug_assertions)` — it means a focusable field's draw site is
    /// missing its stamp call entirely, a coding-time bug that should be
    /// caught in development rather than shipped. `false` when nothing is
    /// focused.
    pub fn focused_was_never_stamped(&self) -> bool {
        let Some(id) = self.focused.as_ref() else { return false };
        self.fields.get(id).map(|s| s.last_frame == 0).unwrap_or(false)
    }

    /// Whether the cursor should be visible right now (500 ms blink).
    pub fn cursor_visible(&self, now_ms: u64) -> bool {
        let elapsed = now_ms.wrapping_sub(self.blink_reset_time);
        (elapsed / 500) % 2 == 0
    }

    /// Shared read access to a field's full state.
    pub fn field_state(&self, id: &WidgetId) -> Option<&TextFieldState> {
        self.fields.get(id)
    }

    /// Exclusive access to a field's full state.
    pub fn field_state_mut(&mut self, id: &WidgetId) -> Option<&mut TextFieldState> {
        self.fields.get_mut(id)
    }

    /// Whether a field is registered.
    pub fn has_field(&self, id: &WidgetId) -> bool {
        self.fields.contains_key(id)
    }

    // =========================================================================
    // Focus
    // =========================================================================

    /// Focus a field. Returns `true` if focus changed.
    pub fn focus(&mut self, id: impl Into<WidgetId>) -> bool {
        let id = id.into();
        if self.focused.as_ref() == Some(&id) {
            // Already focused, so focus does not change — but this is still an
            // ARMING call, and arming is silent. A dialog that closes without
            // blurring and opens again would otherwise inherit the engagement
            // of its previous life.
            if let Some(state) = self.fields.get_mut(&id) {
                state.engaged = false;
            }
            return false;
        }
        if let Some(prev) = self.focused.take() {
            if let Some(state) = self.fields.get_mut(&prev) {
                state.selection_start = None;
            }
        }
        if let Some(state) = self.fields.get_mut(&id) {
            // Arming is silent. Whoever focused this field did not necessarily
            // ask for it to look focused; typing or clicking is what does.
            state.engaged = false;
        }
        self.focused = Some(id);
        self.reset_blink();
        true
    }

    /// Remove focus from the currently focused field.
    pub fn blur(&mut self) {
        if let Some(id) = self.focused.take() {
            if let Some(state) = self.fields.get_mut(&id) {
                state.selection_start = None;
            }
        }
        self.drag_field = None;
    }

    /// Set text programmatically. Positions cursor at end. Does not require focus.
    pub fn set_text(&mut self, id: &WidgetId, text: &str) {
        if let Some(state) = self.fields.get_mut(id) {
            state.text = text.to_string();
            state.cursor = state.char_count();
            state.selection_start = None;
        }
    }

    /// Clear text and reset cursor/selection. Does not require focus.
    pub fn clear(&mut self, id: &WidgetId) {
        if let Some(state) = self.fields.get_mut(id) {
            state.text.clear();
            state.cursor = 0;
            state.selection_start = None;
        }
    }

    /// Snapshot current text as `original_text` so Cancel can revert to it.
    pub fn begin_edit(&mut self, id: &WidgetId) {
        if let Some(state) = self.fields.get_mut(id) {
            state.original_text = state.text.clone();
        }
    }

    /// Override the blink reset timestamp from the platform clock.
    pub fn set_blink_time(&mut self, now_ms: u64) {
        self.blink_reset_time = now_ms;
    }

    // =========================================================================
    // Input dispatch
    // =========================================================================

    /// Handle a printable character.
    pub fn on_char(&mut self, ch: char) -> TextAction {
        let id = match self.focused.clone() {
            Some(id) => id,
            None => return TextAction::None,
        };

        // Raw fields bypass all text-editing logic.
        if self.fields.get(&id).map(|s| s.config.capability == InputCapability::Raw).unwrap_or(false) {
            return TextAction::RawInput(raw_char_to_bytes(ch));
        }

        let state = match self.fields.get_mut(&id) {
            Some(s) => s,
            None => return TextAction::None,
        };
        // Typing is engagement: the field becomes visibly focused the
        // moment it receives a keystroke, not when it was armed.
        state.engaged = true;

        // Mouse-only or read-only fields reject char input.
        if state.config.capability == InputCapability::Mouse || state.config.read_only {
            return TextAction::None;
        }

        match ch {
            '\r' | '\n' => {
                let text = state.text.clone();
                TextAction::Commit(text)
            }
            '\x1b' => {
                let original = state.original_text.clone();
                state.text = original;
                state.cursor = state.char_count();
                state.selection_start = None;
                self.reset_blink();
                TextAction::Cancel
            }
            '\x08' => {
                if state.selection_range().is_some() {
                    state.delete_selection();
                } else if state.cursor > 0 {
                    let byte_pos = state.char_to_byte(state.cursor - 1);
                    let byte_end = state.char_to_byte(state.cursor);
                    state.text.drain(byte_pos..byte_end);
                    state.cursor -= 1;
                }
                self.reset_blink();
                let text = self.fields[&id].text.clone();
                TextAction::TextChanged(text)
            }
            c if c.is_control() => TextAction::None,
            c => {
                if let Some(filter) = state.config.char_filter {
                    if !filter(c) {
                        return TextAction::None;
                    }
                }
                let text_len_after_delete = if state.selection_range().is_some() {
                    let (lo, hi) = state.selection_range().unwrap();
                    state.char_count() - (hi - lo)
                } else {
                    state.char_count()
                };
                if let Some(max) = state.config.max_len {
                    if text_len_after_delete >= max {
                        return TextAction::None;
                    }
                }
                if state.selection_range().is_some() {
                    state.delete_selection();
                }
                let byte_pos = state.char_to_byte(state.cursor);
                state.text.insert(byte_pos, c);
                state.cursor += 1;
                self.reset_blink();
                let text = self.fields[&id].text.clone();
                TextAction::TextChanged(text)
            }
        }
    }

    /// Handle a named key press.
    pub fn on_key(&mut self, key: KeyPress) -> TextAction {
        let id = match self.focused.clone() {
            Some(id) => id,
            None => return TextAction::None,
        };

        // Raw fields bypass all text-editing logic.
        if self.fields.get(&id).map(|s| s.config.capability == InputCapability::Raw).unwrap_or(false) {
            if let Some(bytes) = key_to_pty_bytes(&key) {
                return TextAction::RawInput(bytes);
            }
            return TextAction::None;
        }

        let state = match self.fields.get_mut(&id) {
            Some(s) => s,
            None => return TextAction::None,
        };
        // Typing is engagement: the field becomes visibly focused the
        // moment it receives a keystroke, not when it was armed.
        state.engaged = true;

        let restricted = state.config.capability == InputCapability::Mouse || state.config.read_only;
        if restricted {
            match &key {
                KeyPress::Copy | KeyPress::SelectAll => {}
                _ => return TextAction::None,
            }
        }

        let text_before = state.text.clone();
        let consumed = apply_key(state, key);
        if consumed {
            self.reset_blink();
        }
        let text_after = &self.fields[&id].text;
        if text_after != &text_before {
            TextAction::TextChanged(text_after.clone())
        } else {
            TextAction::None
        }
    }

    /// Begin a mouse drag on the field whose rect contains `(x, y)`.
    ///
    /// Geometrically hit-tests every registered field's last-drawn rect
    /// (`TextFieldState::last_rect`) and, if one is hit, delegates to
    /// [`Self::begin_drag_at`]. Use this when the caller does NOT already
    /// know which field was hit (e.g. a raw platform mousedown handler with
    /// no z-order-aware coordinator of its own). When the caller already
    /// knows the hit field — e.g. `InputCoordinator::end_frame`, which just
    /// ran its own z-order-aware hit test — call [`Self::begin_drag_at`]
    /// directly instead; it skips this geometric re-search entirely, so it
    /// works even on the very first frame a field is registered, before any
    /// draw call has stamped a `last_rect` for it via [`Self::update_field`].
    pub fn on_drag_start(&mut self, x: f64, y: f64) {
        let hit_id = self.hit_test_field_rect(x, y);
        match hit_id {
            Some(id) => self.begin_drag_at(id, x),
            None => {
                // A press that landed on nothing editable ENDS engagement. The
                // field stays armed for the keyboard, but it stops looking
                // live: clicking a dialog's header, or anywhere else outside
                // the box, must take the caret and the ring away. Without this
                // the ring survived every click that was not a close.
                for state in self.fields.values_mut() {
                    state.engaged = false;
                }
            }
        }
    }

    /// Geometric hit-test against every registered field's last-drawn rect,
    /// skipping mouse-incapable fields and fields whose geometry has gone
    /// stale (not stamped via [`Self::update_field`] this frame or the
    /// previous one).
    fn hit_test_field_rect(&self, x: f64, y: f64) -> Option<WidgetId> {
        for (id, state) in &self.fields {
            if state.config.capability == InputCapability::Keyboard {
                continue;
            }
            let frame_lag = self.current_frame.wrapping_sub(state.last_frame);
            if frame_lag > 1 {
                continue;
            }
            if let Some((rx, ry, rw, rh)) = state.last_rect {
                if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                    return Some(id.clone());
                }
            }
        }
        None
    }

    /// Focus `id` and place the caret at the character nearest `x`, arming a
    /// drag-select session on it — the same effect [`Self::on_drag_start`]
    /// has once it has resolved which field was hit, but for a field the
    /// caller already knows (skips the geometric `last_rect` search).
    ///
    /// This is the caret-placement half of a click/press on a text-sense
    /// widget: focusing alone arms the field for the keyboard but leaves the
    /// caret wherever it happened to be from a previous session (or at 0 on
    /// first focus) — calling this instead moves it under the pointer, the
    /// way every native text field behaves on click.
    pub fn begin_drag_at(&mut self, id: impl Into<WidgetId>, x: f64) {
        let id = id.into();
        self.focus(id.clone());

        let state = match self.fields.get_mut(&id) {
            Some(s) => s,
            None => return,
        };
        // A click INTO the field is engagement — `focus` above cleared the
        // flag, which is right for an arming focus and wrong for this one.
        state.engaged = true;

        let cursor = cursor_from_x(&state.last_char_positions, x);
        state.cursor = cursor;
        state.selection_start = Some(cursor);
        self.drag_field = Some(id);
        self.reset_blink();
    }

    /// Update the drag-selection cursor as the mouse moves.
    pub fn on_drag_move(&mut self, x: f64) {
        let id = match self.drag_field.clone() {
            Some(id) => id,
            None => return,
        };

        let state = match self.fields.get_mut(&id) {
            Some(s) => s,
            None => return,
        };

        state.cursor = cursor_from_x(&state.last_char_positions, x);
    }

    /// Returns `true` if a text-select drag is currently in progress.
    pub fn drag_active(&self) -> bool {
        self.drag_field.is_some()
    }

    /// End the drag-selection. Clears degenerate (zero-width) selections.
    pub fn on_drag_end(&mut self) {
        if let Some(id) = self.drag_field.clone() {
            if let Some(state) = self.fields.get_mut(&id) {
                if state.selection_start == Some(state.cursor) {
                    state.selection_start = None;
                }
            }
        }
        self.drag_field = None;
    }

    /// Return the selected text of the focused (or drag) field for the clipboard.
    pub fn copy_selection(&self) -> Option<String> {
        let id = self.focused.as_ref().or(self.drag_field.as_ref())?;
        let state = self.fields.get(id)?;
        let (lo, hi) = state.selection_range()?;
        let byte_lo = state.char_to_byte(lo);
        let byte_hi = state.char_to_byte(hi);
        Some(state.text[byte_lo..byte_hi].to_string())
    }

    // =========================================================================
    // Private
    // =========================================================================

    fn reset_blink(&mut self) {
        self.blink_reset_time = 0;
    }
}

impl Default for TextFieldStore {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Free helpers (private)
// =============================================================================

/// Apply a key event to a `TextFieldState`. Returns `true` if consumed.
fn apply_key(state: &mut TextFieldState, key: KeyPress) -> bool {
    let char_count = state.char_count();

    match key {
        KeyPress::Delete => {
            if state.selection_range().is_some() {
                state.delete_selection();
            } else if state.cursor < char_count {
                let byte_idx = state.char_to_byte(state.cursor);
                state.text.remove(byte_idx);
            }
            true
        }
        KeyPress::ArrowLeft => {
            if state.selection_range().is_some() {
                let (lo, _) = state.selection_range().unwrap();
                state.cursor = lo;
                state.selection_start = None;
            } else {
                state.cursor = state.cursor.saturating_sub(1);
            }
            true
        }
        KeyPress::ArrowRight => {
            if state.selection_range().is_some() {
                let (_, hi) = state.selection_range().unwrap();
                state.cursor = hi;
                state.selection_start = None;
            } else if state.cursor < char_count {
                state.cursor += 1;
            }
            true
        }
        KeyPress::Home => {
            state.cursor = 0;
            state.selection_start = None;
            true
        }
        KeyPress::End => {
            state.cursor = char_count;
            state.selection_start = None;
            true
        }
        KeyPress::SelectAll => {
            state.selection_start = Some(0);
            state.cursor = char_count;
            true
        }
        KeyPress::ShiftLeft => {
            if state.selection_start.is_none() {
                state.selection_start = Some(state.cursor);
            }
            state.cursor = state.cursor.saturating_sub(1);
            if state.selection_start == Some(state.cursor) {
                state.selection_start = None;
            }
            true
        }
        KeyPress::ShiftRight => {
            if state.selection_start.is_none() {
                state.selection_start = Some(state.cursor);
            }
            if state.cursor < char_count {
                state.cursor += 1;
            }
            if state.selection_start == Some(state.cursor) {
                state.selection_start = None;
            }
            true
        }
        KeyPress::ShiftHome => {
            if state.selection_start.is_none() {
                state.selection_start = Some(state.cursor);
            }
            state.cursor = 0;
            if state.selection_start == Some(state.cursor) {
                state.selection_start = None;
            }
            true
        }
        KeyPress::ShiftEnd => {
            if state.selection_start.is_none() {
                state.selection_start = Some(state.cursor);
            }
            state.cursor = char_count;
            if state.selection_start == Some(state.cursor) {
                state.selection_start = None;
            }
            true
        }
        KeyPress::Copy => false,
        KeyPress::Paste(ref text) => {
            if state.config.read_only {
                return false;
            }
            if state.selection_range().is_some() {
                state.delete_selection();
            }
            for ch in text.chars() {
                if ch.is_control() {
                    continue;
                }
                if let Some(filter) = state.config.char_filter {
                    if !filter(ch) {
                        continue;
                    }
                }
                if let Some(max) = state.config.max_len {
                    if state.char_count() >= max {
                        break;
                    }
                }
                let byte_pos = state.char_to_byte(state.cursor);
                state.text.insert(byte_pos, ch);
                state.cursor += 1;
            }
            true
        }
        KeyPress::Undo | KeyPress::Redo => false,
        KeyPress::ArrowUp
        | KeyPress::ArrowDown
        | KeyPress::Enter
        | KeyPress::Escape
        | KeyPress::Tab
        | KeyPress::Backspace
        | KeyPress::CtrlC
        | KeyPress::PageUp
        | KeyPress::PageDown => false,
    }
}

/// Encode a printable character as PTY bytes.
fn raw_char_to_bytes(ch: char) -> Vec<u8> {
    if ch == '\r' || ch == '\n' {
        return vec![b'\r'];
    }
    if ch == '\x08' {
        return vec![0x7f];
    }
    if ch == '\x1b' {
        return vec![0x1b];
    }
    if (ch as u32) < 0x20 {
        return vec![ch as u8];
    }
    let mut buf = [0u8; 4];
    let s = ch.encode_utf8(&mut buf);
    s.as_bytes().to_vec()
}

/// Map named key presses to ANSI escape sequences for PTY forwarding.
fn key_to_pty_bytes(key: &KeyPress) -> Option<Vec<u8>> {
    match key {
        KeyPress::ArrowLeft  => Some(b"\x1b[D".to_vec()),
        KeyPress::ArrowRight => Some(b"\x1b[C".to_vec()),
        KeyPress::ArrowUp    => Some(b"\x1b[A".to_vec()),
        KeyPress::ArrowDown  => Some(b"\x1b[B".to_vec()),
        KeyPress::Home       => Some(b"\x1b[H".to_vec()),
        KeyPress::End        => Some(b"\x1b[F".to_vec()),
        KeyPress::Delete     => Some(b"\x1b[3~".to_vec()),
        KeyPress::PageUp     => Some(b"\x1b[5~".to_vec()),
        KeyPress::PageDown   => Some(b"\x1b[6~".to_vec()),
        KeyPress::Enter      => Some(b"\r".to_vec()),
        KeyPress::Escape     => Some(b"\x1b".to_vec()),
        KeyPress::Tab        => Some(b"\t".to_vec()),
        KeyPress::Backspace  => Some(b"\x7f".to_vec()),
        KeyPress::CtrlC      => Some(b"\x03".to_vec()),
        KeyPress::ShiftLeft  => Some(b"\x1b[1;2D".to_vec()),
        KeyPress::ShiftRight => Some(b"\x1b[1;2C".to_vec()),
        _ => None,
    }
}

/// Compute cursor char index from x position using char boundary positions.
fn cursor_from_x(positions: &[f64], x: f64) -> usize {
    if positions.is_empty() {
        return 0;
    }
    let char_count = positions.len().saturating_sub(1);
    for i in 0..char_count {
        let left = positions[i];
        let right = positions[i + 1];
        let mid = (left + right) * 0.5;
        if x < mid {
            return i;
        }
    }
    char_count
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> TextFieldStore {
        TextFieldStore::new()
    }

    fn id(s: &str) -> WidgetId {
        WidgetId::new(s)
    }

    #[test]
    fn register_and_focus() {
        let mut s = store();
        s.register("a", TextFieldConfig::text());
        assert!(s.has_field(&id("a")));
        assert!(!s.is_focused(&id("a")));
        s.focus("a");
        assert!(s.is_focused(&id("a")));
    }

    #[test]
    fn char_input() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        let action = s.on_char('x');
        assert_eq!(action, TextAction::TextChanged("x".into()));
        assert_eq!(s.text(&id("f")), "x");
        assert_eq!(s.cursor(&id("f")), 1);
    }

    #[test]
    fn char_filter() {
        let mut s = store();
        s.register("f", TextFieldConfig::text().with_filter(|c| c.is_ascii_digit()));
        s.focus("f");
        let _ = s.on_char('5');
        let _ = s.on_char('Z'); // rejected
        assert_eq!(s.text(&id("f")), "5");
    }

    #[test]
    fn commit_cancel() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        s.set_text(&id("f"), "hello");
        s.begin_edit(&id("f"));
        s.on_char('X');
        assert_eq!(s.text(&id("f")), "helloX");
        let action = s.on_char('\x1b');
        assert_eq!(action, TextAction::Cancel);
        assert_eq!(s.text(&id("f")), "hello");

        s.set_text(&id("f"), "hi");
        s.begin_edit(&id("f"));
        let action = s.on_char('\r');
        assert_eq!(action, TextAction::Commit("hi".into()));
    }

    #[test]
    fn blink() {
        let mut s = store();
        // blink_reset_time = 0, now = 0 → elapsed = 0 → visible
        assert!(s.cursor_visible(0));
        // elapsed = 600 → phase 1 → hidden
        assert!(!s.cursor_visible(600));
        // elapsed = 1100 → phase 2 → visible
        assert!(s.cursor_visible(1100));
        s.set_blink_time(1000);
        // now = 1000 → elapsed = 0 → visible
        assert!(s.cursor_visible(1000));
    }

    #[test]
    fn selection_and_delete() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        s.set_text(&id("f"), "hello");
        // SelectAll
        s.on_key(KeyPress::SelectAll);
        assert_eq!(s.selection_range(&id("f")), Some((0, 5)));
        // Delete selection
        s.on_key(KeyPress::Delete);
        assert_eq!(s.text(&id("f")), "");
        assert_eq!(s.cursor(&id("f")), 0);
    }

    #[test]
    fn raw_mode() {
        let mut s = store();
        s.register("pty", TextFieldConfig::raw());
        s.focus("pty");
        let action = s.on_char('a');
        assert_eq!(action, TextAction::RawInput(vec![b'a']));
        let action = s.on_key(KeyPress::ArrowUp);
        assert_eq!(action, TextAction::RawInput(b"\x1b[A".to_vec()));
    }

    #[test]
    fn read_only() {
        let mut s = store();
        s.register("ro", TextFieldConfig::read_only());
        s.focus("ro");
        s.set_text(&id("ro"), "fixed");
        let action = s.on_char('X');
        assert_eq!(action, TextAction::None);
        assert_eq!(s.text(&id("ro")), "fixed");
        // SelectAll still works
        let action = s.on_key(KeyPress::SelectAll);
        assert_eq!(action, TextAction::None);
        assert_eq!(s.selection_range(&id("ro")), Some((0, 5)));
    }

    #[test]
    fn focused_is_stale_eventually_catches_a_never_stamped_field() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        // A field whose host never calls update_field/mark_seen at all
        // goes stale the same way one whose stamps stopped does — a
        // focusable field with no stamp call anywhere is a bug, not a
        // safe default to trust forever.
        for _ in 0..10 {
            s.begin_frame();
        }
        assert!(s.focused_is_stale());
    }

    #[test]
    fn mark_seen_keeps_a_button_style_field_fresh_without_touching_geometry() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        for _ in 0..10 {
            s.begin_frame();
            s.mark_seen(&id("f"));
        }
        assert!(!s.focused_is_stale());
        // mark_seen never sets rect/char_positions — a field stamped only
        // this way must never become hit-testable by on_drag_start.
        assert!(s.field_state(&id("f")).unwrap().last_rect.is_none());
        assert!(s.field_state(&id("f")).unwrap().last_char_positions.is_empty());
    }

    #[test]
    fn mark_seen_clears_the_never_stamped_flag() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        assert!(s.focused_was_never_stamped());
        // current_frame must have advanced past 0 for the stamp to be
        // distinguishable from "never stamped" (both read as last_frame == 0
        // at the very first tick) — an ordinary host has rendered many
        // frames before the first keystroke ever reaches dispatch.
        s.begin_frame();
        s.mark_seen(&id("f"));
        assert!(!s.focused_was_never_stamped());
    }

    #[test]
    fn focused_was_never_stamped_true_immediately_after_focus() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        assert!(!s.focused_was_never_stamped(), "nothing focused yet");
        s.focus("f");
        assert!(s.focused_was_never_stamped());
    }

    #[test]
    fn focused_was_never_stamped_false_once_update_field_lands() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        s.begin_frame();
        s.update_field(&id("f"), (0.0, 0.0, 10.0, 10.0), vec![0.0, 10.0]);
        assert!(!s.focused_was_never_stamped());
    }

    #[test]
    fn focused_was_never_stamped_false_when_nothing_focused() {
        let s = store();
        assert!(!s.focused_was_never_stamped());
    }

    #[test]
    fn focused_is_stale_false_when_nothing_focused() {
        let s = store();
        assert!(!s.focused_is_stale());
    }

    #[test]
    fn focused_is_stale_detects_field_that_stopped_rendering() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        s.begin_frame();
        s.update_field(&id("f"), (0.0, 0.0, 10.0, 10.0), vec![0.0, 10.0]);
        // Freshly drawn — not stale.
        assert!(!s.focused_is_stale());

        // A couple of frames within the tolerance (the double begin_frame
        // per render pass a real host does) — still not stale.
        s.begin_frame();
        s.begin_frame();
        assert!(!s.focused_is_stale());

        // The field's owner stops being drawn (e.g. its modal closed)
        // while focus lingers — eventually detected as stale.
        for _ in 0..5 {
            s.begin_frame();
        }
        assert!(s.focused_is_stale());
    }

    #[test]
    fn unregister() {
        let mut s = store();
        s.register("a", TextFieldConfig::text());
        s.focus("a");
        s.unregister(&id("a"));
        assert!(!s.has_field(&id("a")));
        assert!(s.focused().is_none());
    }

    #[test]
    fn drag_select() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        s.set_text(&id("f"), "hello");
        s.begin_frame();
        // positions: 0..5 chars, each 10px wide
        let positions: Vec<f64> = (0..=5).map(|i| i as f64 * 10.0).collect();
        s.update_field(&id("f"), (0.0, 0.0, 50.0, 20.0), positions);
        // drag_start at x=5 → char 0 (mid of char 0 is 5, x < 5 → char 0, x==5 → char 1)
        s.on_drag_start(3.0, 5.0);
        let anchor = s.field_state(&id("f")).unwrap().selection_start;
        assert!(anchor.is_some());
        // drag_move to x=35 → char 3 (mid of char 3 is 35, x<35 → 3)
        s.on_drag_move(25.0);
        let cursor_after = s.cursor(&id("f"));
        assert_ne!(cursor_after, anchor.unwrap());
        s.on_drag_end();
        assert!(s.selection_range(&id("f")).is_some());
    }

    #[test]
    fn begin_drag_at_places_caret_without_geometric_search() {
        // A caller that already knows which field was hit (e.g. a z-order-
        // aware coordinator) can go straight through `begin_drag_at` — this
        // must work even before the field has ever been drawn (no
        // `update_field` call yet, so `last_rect` is `None` and the
        // geometric search `on_drag_start` relies on would find nothing).
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.set_text(&id("f"), "hello");
        assert_eq!(s.cursor(&id("f")), 5);

        // No update_field call — last_char_positions is empty, so
        // cursor_from_x degrades to 0 rather than panicking.
        s.begin_drag_at("f", 12.0);
        assert!(s.is_focused(&id("f")));
        assert_eq!(s.cursor(&id("f")), 0);
        assert!(s.is_engaged(&id("f")));

        // Once geometry is known, the caret lands on the character nearest x.
        let positions: Vec<f64> = (0..=5).map(|i| i as f64 * 10.0).collect();
        s.update_field(&id("f"), (0.0, 0.0, 50.0, 20.0), positions);
        s.begin_drag_at("f", 25.0);
        assert_eq!(s.cursor(&id("f")), 3);
        // A fresh press collapses any prior selection to a point at the hit char.
        assert_eq!(s.field_state(&id("f")).unwrap().selection_start, Some(3));
    }

    #[test]
    fn max_len_enforced() {
        let mut s = store();
        s.register("f", TextFieldConfig::text().with_max_len(3));
        s.focus("f");
        s.on_char('a');
        s.on_char('b');
        s.on_char('c');
        let action = s.on_char('d');
        assert_eq!(action, TextAction::None);
        assert_eq!(s.text(&id("f")), "abc");
    }

    #[test]
    fn utf8_cursor_arithmetic() {
        let mut s = store();
        s.register("f", TextFieldConfig::text());
        s.focus("f");
        s.on_char('А'); // Cyrillic, 2-byte UTF-8
        s.on_char('Б');
        s.on_char('В');
        assert_eq!(s.text(&id("f")).chars().count(), 3);
        assert_eq!(s.cursor(&id("f")), 3);
        // Backspace
        s.on_char('\x08');
        assert_eq!(s.text(&id("f")).chars().count(), 2);
        assert_eq!(s.cursor(&id("f")), 2);
    }
}
