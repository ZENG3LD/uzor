//! Text input behavior types - validation rules and action results

/// Validation and confirmation behavior for text fields
#[derive(Clone, Debug, PartialEq)]
pub enum TextFieldBehavior {
    /// Accepts all printable characters. Enter returns text as-is.
    FreeText,

    /// Accepts digits, '.', and optional '-'. Enter parses as f64 with clamping.
    NumericFloat { min: f64, max: f64 },

    /// Accepts digits only (no negative support for now).
    /// Enter parses as u32 with clamping.
    NumericInt { min: u32, max: u32 },

    /// Search field. Accepts all printable chars. Always live-updates.
    Search,
}

/// Configuration for a text field instance
#[derive(Clone, Debug)]
pub struct TextFieldConfig {
    /// Field identifier (must match the field_id used in start_editing)
    pub field_id: String,

    /// Input validation and confirmation behavior
    pub behavior: TextFieldBehavior,

    /// Whether to emit Changed action on every keystroke
    pub live_update: bool,
}

impl TextFieldConfig {
    /// Create a new text field config
    pub fn new(field_id: &str, behavior: TextFieldBehavior) -> Self {
        let live_update = matches!(behavior, TextFieldBehavior::Search);
        Self {
            field_id: field_id.to_string(),
            behavior,
            live_update,
        }
    }

    /// Set live_update flag
    pub fn with_live_update(mut self, live: bool) -> Self {
        self.live_update = live;
        self
    }
}

/// Key events for text input processing
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TextInputKey {
    // Navigation
    Left,
    Right,
    Home,
    End,

    // Deletion
    Backspace,
    Delete,

    // Confirmation
    Enter,
    Escape,

    // Selection
    SelectAll,  // Ctrl+A

    // Focus management (for InputCoordinator)
    Tab,

    // Clipboard
    Copy,       // Ctrl+C
    Cut,        // Ctrl+X
    Paste(String), // Ctrl+V with clipboard text
}

/// Result of processing a char or key event
#[derive(Clone, Debug)]
pub enum TextInputAction {
    /// Input consumed, state updated internally. No further action needed.
    Consumed,

    /// Text changed during editing (for live-update fields).
    Changed(String),

    /// Enter pressed and value parsed successfully.
    Confirmed(ConfirmedValue),

    /// Editing cancelled (Escape) or parse failed (invalid Enter).
    /// Contains original text for restoration.
    Cancelled(String),

    /// Tab pressed — coordinator should move focus to next field.
    FocusNext,

    /// Shift+Tab pressed — coordinator should move focus to previous field.
    FocusPrev,

    /// Input was not consumed (field not active or event not relevant).
    NotConsumed,
}

/// Parsed value from Enter confirmation
#[derive(Clone, Debug, PartialEq)]
pub enum ConfirmedValue {
    Text(String),
    Float(f64),
    Int(u32),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_auto_live_update_for_search() {
        let config = TextFieldConfig::new("search", TextFieldBehavior::Search);
        assert!(config.live_update);
    }

    #[test]
    fn test_config_manual_live_update() {
        let config = TextFieldConfig::new("text", TextFieldBehavior::FreeText)
            .with_live_update(true);
        assert!(config.live_update);
    }
}
