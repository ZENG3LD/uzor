//! Text input handler - Event processing and validation

use crate::types::Rect;
use super::behavior::{TextFieldBehavior, TextFieldConfig, TextInputAction, TextInputKey, ConfirmedValue};
use super::state::TextInputState;

/// Keyboard modifiers state
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

/// Text input event handler
pub trait TextInputHandler {
    // =========================================================================
    // Hit testing and cursor positioning (low-level, keep for rendering)
    // =========================================================================

    fn hit_test(&self, input_rect: Rect, mouse_pos: (f64, f64)) -> bool {
        let (x, y) = mouse_pos;
        x >= input_rect.x
            && x <= input_rect.x + input_rect.width
            && y >= input_rect.y
            && y <= input_rect.y + input_rect.height
    }

    fn mouse_to_cursor_position(
        &self,
        mouse_x: f64,
        text_x: f64,
        text: &str,
        font_size: f64,
    ) -> usize {
        let char_width = font_size * 0.6;
        let relative_x = mouse_x - text_x;

        if relative_x <= 0.0 {
            return 0;
        }

        let mut x = 0.0;
        for (i, _) in text.char_indices() {
            x += char_width;
            if relative_x < x - char_width / 2.0 {
                return i;
            }
        }

        text.chars().count()
    }

    // =========================================================================
    // High-level processing (NEW: core text input logic)
    // =========================================================================

    /// Process a character input event for the active text field.
    ///
    /// Validates the character against the field's behavior (numeric filtering, etc.),
    /// inserts it if valid, and returns the appropriate action.
    fn process_char(
        &self,
        state: &mut TextInputState,
        config: &TextFieldConfig,
        c: char,
    ) -> TextInputAction {
        // Check if this field is active
        if !state.is_editing(&config.field_id) {
            return TextInputAction::NotConsumed;
        }

        // Skip control characters
        if c.is_control() {
            return TextInputAction::Consumed;
        }

        // Validate against behavior
        if !self.is_valid_char(&config.behavior, c, &state.text, state.cursor) {
            return TextInputAction::Consumed; // Swallow invalid chars silently
        }

        state.insert_char(c);

        if config.live_update {
            TextInputAction::Changed(state.text.clone())
        } else {
            TextInputAction::Consumed
        }
    }

    /// Process a key input event for the active text field.
    ///
    /// Handles navigation (arrows, home, end), deletion (backspace, delete),
    /// selection (shift+arrows, select all), confirmation (Enter), and
    /// focus management (Tab).
    fn process_key(
        &self,
        state: &mut TextInputState,
        config: &TextFieldConfig,
        key: TextInputKey,
        shift: bool,
    ) -> TextInputAction {
        // Check if this field is active
        if !state.is_editing(&config.field_id) {
            return TextInputAction::NotConsumed;
        }

        match key {
            TextInputKey::Enter => {
                let text = match state.finish_editing() {
                    Some(t) => t,
                    None => return TextInputAction::NotConsumed,
                };
                self.parse_and_confirm(&config.behavior, text)
            }

            TextInputKey::Escape => {
                let original = state.cancel_editing()
                    .unwrap_or_default();
                TextInputAction::Cancelled(original)
            }

            TextInputKey::Backspace => {
                state.backspace();
                if config.live_update {
                    TextInputAction::Changed(state.text.clone())
                } else {
                    TextInputAction::Consumed
                }
            }

            TextInputKey::Delete => {
                state.delete();
                if config.live_update {
                    TextInputAction::Changed(state.text.clone())
                } else {
                    TextInputAction::Consumed
                }
            }

            TextInputKey::Left => {
                state.move_left(shift);
                TextInputAction::Consumed
            }

            TextInputKey::Right => {
                state.move_right(shift);
                TextInputAction::Consumed
            }

            TextInputKey::Home => {
                state.move_home(shift);
                TextInputAction::Consumed
            }

            TextInputKey::End => {
                state.move_end(shift);
                TextInputAction::Consumed
            }

            TextInputKey::SelectAll => {
                state.select_all();
                TextInputAction::Consumed
            }

            TextInputKey::Tab => {
                if shift {
                    TextInputAction::FocusPrev
                } else {
                    TextInputAction::FocusNext
                }
            }

            TextInputKey::Copy => {
                if let Some(_text) = state.get_selected_text() {
                    // Return the text via Consumed for now
                    // In future, could have ClipboardCopy(String) action
                    TextInputAction::Consumed
                } else {
                    TextInputAction::Consumed
                }
            }

            TextInputKey::Cut => {
                if let Some(_text) = state.cut() {
                    // Return the text via action if live_update
                    if config.live_update {
                        TextInputAction::Changed(state.text.clone())
                    } else {
                        TextInputAction::Consumed
                    }
                } else {
                    TextInputAction::Consumed
                }
            }

            TextInputKey::Paste(ref text) => {
                state.paste(text);
                if config.live_update {
                    TextInputAction::Changed(state.text.clone())
                } else {
                    TextInputAction::Consumed
                }
            }
        }
    }

    // =========================================================================
    // Validation helpers
    // =========================================================================

    /// Check if a character is valid for the given behavior
    fn is_valid_char(
        &self,
        behavior: &TextFieldBehavior,
        c: char,
        current_text: &str,
        cursor: usize,
    ) -> bool {
        match behavior {
            TextFieldBehavior::FreeText | TextFieldBehavior::Search => {
                c.is_ascii_graphic() || c == ' '
            }
            TextFieldBehavior::NumericFloat { .. } => {
                c.is_ascii_digit()
                    || c == '.'
                    || (c == '-' && cursor == 0 && !current_text.contains('-'))
            }
            TextFieldBehavior::NumericInt { .. } => c.is_ascii_digit(),
        }
    }

    /// Parse text according to behavior and return confirmed value or error
    fn parse_and_confirm(
        &self,
        behavior: &TextFieldBehavior,
        text: String,
    ) -> TextInputAction {
        match behavior {
            TextFieldBehavior::FreeText | TextFieldBehavior::Search => {
                TextInputAction::Confirmed(ConfirmedValue::Text(text))
            }
            TextFieldBehavior::NumericFloat { min, max } => {
                match text.parse::<f64>() {
                    Ok(v) => TextInputAction::Confirmed(
                        ConfirmedValue::Float(v.clamp(*min, *max))
                    ),
                    Err(_) => TextInputAction::Cancelled(text),
                }
            }
            TextFieldBehavior::NumericInt { min, max } => {
                match text.parse::<u32>() {
                    Ok(v) => TextInputAction::Confirmed(
                        ConfirmedValue::Int(v.clamp(*min, *max))
                    ),
                    Err(_) => TextInputAction::Cancelled(text),
                }
            }
        }
    }

    // =========================================================================
    // Legacy helpers (keep for backward compatibility)
    // =========================================================================

    fn validate_number(&self, text: &str) -> bool {
        if text.is_empty() {
            return true;
        }

        let mut chars = text.chars().peekable();
        let mut has_decimal = false;
        let mut has_digit = false;

        if chars.peek() == Some(&'-') {
            chars.next();
        }

        for c in chars {
            if c.is_ascii_digit() {
                has_digit = true;
            } else if c == '.' {
                if has_decimal {
                    return false;
                }
                has_decimal = true;
            } else {
                return false;
            }
        }

        has_digit || text == "-" || text.is_empty()
    }

    fn mask_password(&self, text: &str) -> String {
        "\u{2022}".repeat(text.chars().count())
    }
}

/// Default implementation of TextInputHandler
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultTextInputHandler;

impl TextInputHandler for DefaultTextInputHandler {}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state(field_id: &str, text: &str) -> TextInputState {
        let mut s = TextInputState::new();
        s.start_editing(field_id, text);
        s
    }

    #[test]
    fn test_free_text_accepts_all_printable() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("test", "");
        let config = TextFieldConfig::new("test", TextFieldBehavior::FreeText);

        let action = handler.process_char(&mut state, &config, 'a');
        assert!(matches!(action, TextInputAction::Consumed));
        assert_eq!(state.get_text(), "a");

        let action = handler.process_char(&mut state, &config, ' ');
        assert!(matches!(action, TextInputAction::Consumed));
        assert_eq!(state.get_text(), "a ");

        let action = handler.process_char(&mut state, &config, '!');
        assert!(matches!(action, TextInputAction::Consumed));
        assert_eq!(state.get_text(), "a !");
    }

    #[test]
    fn test_numeric_float_filters() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("num", "");
        let config = TextFieldConfig::new("num", TextFieldBehavior::NumericFloat { min: 0.0, max: 100.0 });

        handler.process_char(&mut state, &config, '1');
        handler.process_char(&mut state, &config, '.');
        handler.process_char(&mut state, &config, '5');
        assert_eq!(state.get_text(), "1.5");

        // Letters rejected
        handler.process_char(&mut state, &config, 'a');
        assert_eq!(state.get_text(), "1.5");
    }

    #[test]
    fn test_numeric_int_filters() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("num", "");
        let config = TextFieldConfig::new("num", TextFieldBehavior::NumericInt { min: 1, max: 100 });

        handler.process_char(&mut state, &config, '4');
        handler.process_char(&mut state, &config, '2');
        assert_eq!(state.get_text(), "42");

        // Dot rejected for int
        handler.process_char(&mut state, &config, '.');
        assert_eq!(state.get_text(), "42");
    }

    #[test]
    fn test_enter_confirms_float() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("num", "1.5");
        let config = TextFieldConfig::new("num", TextFieldBehavior::NumericFloat { min: 0.0, max: 100.0 });

        let action = handler.process_key(&mut state, &config, TextInputKey::Enter, false);
        match action {
            TextInputAction::Confirmed(ConfirmedValue::Float(v)) => assert_eq!(v, 1.5),
            _ => panic!("Expected Confirmed(Float)"),
        }
    }

    #[test]
    fn test_enter_clamps_value() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("num", "200");
        let config = TextFieldConfig::new("num", TextFieldBehavior::NumericFloat { min: 0.0, max: 100.0 });

        let action = handler.process_key(&mut state, &config, TextInputKey::Enter, false);
        match action {
            TextInputAction::Confirmed(ConfirmedValue::Float(v)) => assert_eq!(v, 100.0),
            _ => panic!("Expected Confirmed(Float)"),
        }
    }

    #[test]
    fn test_enter_invalid_cancels() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("num", "abc");
        let config = TextFieldConfig::new("num", TextFieldBehavior::NumericFloat { min: 0.0, max: 100.0 });

        let action = handler.process_key(&mut state, &config, TextInputKey::Enter, false);
        assert!(matches!(action, TextInputAction::Cancelled(_)));
    }

    #[test]
    fn test_live_update() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("search", "");
        let config = TextFieldConfig::new("search", TextFieldBehavior::Search);

        let action = handler.process_char(&mut state, &config, 'h');
        assert!(matches!(action, TextInputAction::Changed(ref s) if s == "h"));

        let action = handler.process_key(&mut state, &config, TextInputKey::Backspace, false);
        assert!(matches!(action, TextInputAction::Changed(ref s) if s == ""));
    }

    #[test]
    fn test_tab_navigation() {
        let handler = DefaultTextInputHandler;
        let mut state = make_state("test", "hello");
        let config = TextFieldConfig::new("test", TextFieldBehavior::FreeText);

        let action = handler.process_key(&mut state, &config, TextInputKey::Tab, false);
        assert!(matches!(action, TextInputAction::FocusNext));

        let mut state2 = make_state("test", "hello");
        let action = handler.process_key(&mut state2, &config, TextInputKey::Tab, true);
        assert!(matches!(action, TextInputAction::FocusPrev));
    }
}
