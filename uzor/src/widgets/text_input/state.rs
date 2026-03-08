//! Text input state - editing state management

use std::collections::HashMap;

// =============================================================================
// Trait for custom state implementations
// =============================================================================

/// State adapter for text input interaction (trait for custom backends)
pub trait TextInputStateTrait {
    fn is_focused(&self, input_id: &str) -> bool;
    fn cursor_position(&self, input_id: &str) -> usize;
    fn selection_range(&self, input_id: &str) -> Option<(usize, usize)>;
    fn set_focused(&mut self, input_id: &str, focused: bool);
    fn set_cursor_position(&mut self, input_id: &str, pos: usize);
    fn set_selection_range(&mut self, input_id: &str, range: Option<(usize, usize)>);
}

/// Simple HashMap-based implementation (prototyping only)
#[derive(Clone, Debug, Default)]
pub struct SimpleTextInputState {
    pub focus: HashMap<String, bool>,
    pub cursor: HashMap<String, usize>,
    pub selection: HashMap<String, Option<(usize, usize)>>,
}

impl SimpleTextInputState {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TextInputStateTrait for SimpleTextInputState {
    fn is_focused(&self, input_id: &str) -> bool {
        self.focus.get(input_id).copied().unwrap_or(false)
    }

    fn cursor_position(&self, input_id: &str) -> usize {
        self.cursor.get(input_id).copied().unwrap_or(0)
    }

    fn selection_range(&self, input_id: &str) -> Option<(usize, usize)> {
        self.selection.get(input_id).copied().flatten()
    }

    fn set_focused(&mut self, input_id: &str, focused: bool) {
        if focused {
            for (id, is_focused) in self.focus.iter_mut() {
                if id != input_id {
                    *is_focused = false;
                }
            }
        }
        self.focus.insert(input_id.to_string(), focused);
    }

    fn set_cursor_position(&mut self, input_id: &str, pos: usize) {
        self.cursor.insert(input_id.to_string(), pos);
    }

    fn set_selection_range(&mut self, input_id: &str, range: Option<(usize, usize)>) {
        self.selection.insert(input_id.to_string(), range);
    }
}

// =============================================================================
// Concrete TextInputState (MOVED FROM APPLICATION)
// =============================================================================

/// State for an active text input field (standard implementation)
#[derive(Clone, Debug, Default)]
pub struct TextInputState {
    /// Is a text input currently active?
    pub is_active: bool,

    /// Field being edited (e.g., "price_input", "search_box")
    pub field_id: Option<String>,

    /// Current text content being edited
    pub text: String,

    /// Cursor position (character index, NOT byte index)
    pub cursor: usize,

    /// Selection start (if Some, text is selected from selection_start to cursor)
    pub selection_start: Option<usize>,

    /// Original text before editing started (for cancel/undo)
    pub original_text: String,

    /// Timestamp for cursor blink (milliseconds)
    pub blink_time: u64,
}

impl TextInputState {
    /// Create new text input state (inactive)
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if text input is active
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Check if this specific field is being edited
    pub fn is_editing(&self, field_id: &str) -> bool {
        self.is_active && self.field_id.as_deref() == Some(field_id)
    }

    /// Start editing a field with initial text
    pub fn start_editing(&mut self, field_id: &str, initial_text: &str) {
        self.is_active = true;
        self.field_id = Some(field_id.to_string());
        self.text = initial_text.to_string();
        self.cursor = initial_text.chars().count(); // Cursor at end
        self.selection_start = None;
        self.original_text = initial_text.to_string();
    }

    /// Start editing with timestamp for cursor blink
    pub fn start_editing_with_time(&mut self, field_id: &str, initial_text: &str, current_time_ms: u64) {
        self.start_editing(field_id, initial_text);
        self.blink_time = current_time_ms;
    }

    /// Reset blink timer (call when cursor moves or text changes)
    pub fn reset_blink(&mut self, current_time_ms: u64) {
        self.blink_time = current_time_ms;
    }

    /// Check if cursor should be visible based on blink timing
    pub fn is_cursor_visible(&self, current_time_ms: u64) -> bool {
        let elapsed = current_time_ms.wrapping_sub(self.blink_time);
        (elapsed / 500).is_multiple_of(2)
    }

    /// Stop editing and return final text
    pub fn finish_editing(&mut self) -> Option<String> {
        if !self.is_active {
            return None;
        }
        let result = self.text.clone();
        self.clear();
        Some(result)
    }

    /// Cancel editing and restore original text
    pub fn cancel_editing(&mut self) -> Option<String> {
        if !self.is_active {
            return None;
        }
        let result = self.original_text.clone();
        self.clear();
        Some(result)
    }

    /// Clear state (stop editing)
    pub fn clear(&mut self) {
        self.is_active = false;
        self.field_id = None;
        self.text.clear();
        self.cursor = 0;
        self.selection_start = None;
        self.original_text.clear();
        self.blink_time = 0;
    }

    /// Get current text
    pub fn get_text(&self) -> &str {
        &self.text
    }

    /// Get field being edited
    pub fn get_field(&self) -> Option<&str> {
        self.field_id.as_deref()
    }

    // =========================================================================
    // Text manipulation
    // =========================================================================

    /// Insert character at cursor position
    pub fn insert_char(&mut self, c: char) {
        self.delete_selection();
        let byte_pos = self.char_to_byte_pos(self.cursor);
        self.text.insert(byte_pos, c);
        self.cursor += 1;
    }

    /// Insert string at cursor position
    pub fn insert_str(&mut self, s: &str) {
        self.delete_selection();
        let byte_pos = self.char_to_byte_pos(self.cursor);
        self.text.insert_str(byte_pos, s);
        self.cursor += s.chars().count();
    }

    /// Delete character before cursor (backspace)
    pub fn backspace(&mut self) {
        if self.has_selection() {
            self.delete_selection();
        } else if self.cursor > 0 {
            let byte_pos = self.char_to_byte_pos(self.cursor - 1);
            let next_byte_pos = self.char_to_byte_pos(self.cursor);
            self.text.drain(byte_pos..next_byte_pos);
            self.cursor -= 1;
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete(&mut self) {
        if self.has_selection() {
            self.delete_selection();
        } else {
            let char_count = self.text.chars().count();
            if self.cursor < char_count {
                let byte_pos = self.char_to_byte_pos(self.cursor);
                let next_byte_pos = self.char_to_byte_pos(self.cursor + 1);
                self.text.drain(byte_pos..next_byte_pos);
            }
        }
    }

    /// Delete selected text (if any)
    fn delete_selection(&mut self) {
        if let Some(sel_start) = self.selection_start {
            let (start, end) = if sel_start < self.cursor {
                (sel_start, self.cursor)
            } else {
                (self.cursor, sel_start)
            };

            let start_byte = self.char_to_byte_pos(start);
            let end_byte = self.char_to_byte_pos(end);
            self.text.drain(start_byte..end_byte);
            self.cursor = start;
            self.selection_start = None;
        }
    }

    /// Check if there's a selection
    pub fn has_selection(&self) -> bool {
        self.selection_start.is_some() && self.selection_start != Some(self.cursor)
    }

    /// Get selection range (start, end) in character indices
    pub fn get_selection(&self) -> Option<(usize, usize)> {
        self.selection_start.map(|sel_start| {
            if sel_start < self.cursor {
                (sel_start, self.cursor)
            } else {
                (self.cursor, sel_start)
            }
        })
    }

    // =========================================================================
    // Cursor movement
    // =========================================================================

    /// Move cursor left
    pub fn move_left(&mut self, with_selection: bool) {
        if with_selection {
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor);
            }
        } else {
            self.selection_start = None;
        }

        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move cursor right
    pub fn move_right(&mut self, with_selection: bool) {
        if with_selection {
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor);
            }
        } else {
            self.selection_start = None;
        }

        let char_count = self.text.chars().count();
        if self.cursor < char_count {
            self.cursor += 1;
        }
    }

    /// Move cursor to start
    pub fn move_home(&mut self, with_selection: bool) {
        if with_selection {
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor);
            }
        } else {
            self.selection_start = None;
        }
        self.cursor = 0;
    }

    /// Move cursor to end
    pub fn move_end(&mut self, with_selection: bool) {
        if with_selection {
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor);
            }
        } else {
            self.selection_start = None;
        }
        self.cursor = self.text.chars().count();
    }

    /// Select all text
    pub fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor = self.text.chars().count();
    }

    /// Set cursor position at character index (clears selection)
    pub fn set_cursor(&mut self, pos: usize) {
        let char_count = self.text.chars().count();
        self.cursor = pos.min(char_count);
        self.selection_start = None;
    }

    /// Set cursor position based on click X within the text field
    pub fn set_cursor_from_click(&mut self, click_x_offset: f64, char_width: f64) -> usize {
        if char_width <= 0.0 {
            return self.cursor;
        }
        let char_count = self.text.chars().count();
        let clicked_pos = ((click_x_offset / char_width).round() as usize).min(char_count);
        self.cursor = clicked_pos;
        self.selection_start = None;
        clicked_pos
    }

    // =========================================================================
    // Clipboard operations
    // =========================================================================

    /// Get selected text for copy operation
    pub fn get_selected_text(&self) -> Option<String> {
        self.get_selection().map(|(start, end)| {
            let start_byte = self.char_to_byte_pos(start);
            let end_byte = self.char_to_byte_pos(end);
            self.text[start_byte..end_byte].to_string()
        })
    }

    /// Cut selected text (returns text and deletes selection)
    pub fn cut(&mut self) -> Option<String> {
        let text = self.get_selected_text();
        if text.is_some() {
            self.delete_selection();
        }
        text
    }

    /// Paste text at cursor
    pub fn paste(&mut self, text: &str) {
        self.insert_str(text);
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Convert character index to byte position
    fn char_to_byte_pos(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map(|(pos, _)| pos)
            .unwrap_or(self.text.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_editing() {
        let mut state = TextInputState::new();
        state.start_editing("test", "hello");

        assert!(state.is_active());
        assert_eq!(state.get_text(), "hello");
        assert_eq!(state.cursor, 5);

        state.insert_char('!');
        assert_eq!(state.get_text(), "hello!");

        state.backspace();
        assert_eq!(state.get_text(), "hello");
    }

    #[test]
    fn test_cursor_movement() {
        let mut state = TextInputState::new();
        state.start_editing("test", "abc");

        assert_eq!(state.cursor, 3);

        state.move_left(false);
        assert_eq!(state.cursor, 2);

        state.move_home(false);
        assert_eq!(state.cursor, 0);

        state.move_end(false);
        assert_eq!(state.cursor, 3);
    }

    #[test]
    fn test_selection() {
        let mut state = TextInputState::new();
        state.start_editing("test", "hello");

        state.select_all();
        assert_eq!(state.get_selection(), Some((0, 5)));
        assert_eq!(state.get_selected_text(), Some("hello".to_string()));

        state.delete_selection();
        assert_eq!(state.get_text(), "");
    }

    #[test]
    fn test_cancel() {
        let mut state = TextInputState::new();
        state.start_editing("test", "original");

        state.insert_str(" modified");
        assert_eq!(state.get_text(), "original modified");

        let restored = state.cancel_editing();
        assert_eq!(restored, Some("original".to_string()));
        assert!(!state.is_active());
    }
}
