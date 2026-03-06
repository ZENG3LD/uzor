//! Text input type definitions - semantic input variants

/// Main text input type enum covering all input variants
#[derive(Debug, Clone, PartialEq)]
pub enum TextInputType {
    /// Generic text input for strings
    Text {
        value: String,
        placeholder: String,
        focused: bool,
        disabled: bool,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Numeric input with validation
    Number {
        value: String,
        placeholder: String,
        focused: bool,
        disabled: bool,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Search input with search icon
    Search {
        value: String,
        placeholder: String,
        focused: bool,
        position: (f64, f64),
        width: f64,
        height: f64,
    },

    /// Password input with hidden characters
    Password {
        value: String,
        placeholder: String,
        focused: bool,
        position: (f64, f64),
        width: f64,
        height: f64,
    },
}

impl TextInputType {
    pub fn text(placeholder: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Text {
            value: String::new(),
            placeholder: placeholder.into(),
            focused: false,
            disabled: false,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn text_with_value(value: impl Into<String>, placeholder: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Text {
            value: value.into(),
            placeholder: placeholder.into(),
            focused: false,
            disabled: false,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn number(placeholder: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Number {
            value: String::new(),
            placeholder: placeholder.into(),
            focused: false,
            disabled: false,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn number_with_value(value: impl Into<String>, placeholder: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Number {
            value: value.into(),
            placeholder: placeholder.into(),
            focused: false,
            disabled: false,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn search(placeholder: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Search {
            value: String::new(),
            placeholder: placeholder.into(),
            focused: false,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn password(placeholder: impl Into<String>, x: f64, y: f64, width: f64, height: f64) -> Self {
        Self::Password {
            value: String::new(),
            placeholder: placeholder.into(),
            focused: false,
            position: (x, y),
            width,
            height,
        }
    }

    pub fn value(&self) -> &str {
        match self {
            Self::Text { value, .. } => value,
            Self::Number { value, .. } => value,
            Self::Search { value, .. } => value,
            Self::Password { value, .. } => value,
        }
    }

    pub fn is_focused(&self) -> bool {
        match self {
            Self::Text { focused, .. } => *focused,
            Self::Number { focused, .. } => *focused,
            Self::Search { focused, .. } => *focused,
            Self::Password { focused, .. } => *focused,
        }
    }

    pub fn is_disabled(&self) -> bool {
        match self {
            Self::Text { disabled, .. } => *disabled,
            Self::Number { disabled, .. } => *disabled,
            _ => false,
        }
    }

    pub fn position(&self) -> (f64, f64) {
        match self {
            Self::Text { position, .. } => *position,
            Self::Number { position, .. } => *position,
            Self::Search { position, .. } => *position,
            Self::Password { position, .. } => *position,
        }
    }

    pub fn width(&self) -> f64 {
        match self {
            Self::Text { width, .. } => *width,
            Self::Number { width, .. } => *width,
            Self::Search { width, .. } => *width,
            Self::Password { width, .. } => *width,
        }
    }

    pub fn height(&self) -> f64 {
        match self {
            Self::Text { height, .. } => *height,
            Self::Number { height, .. } => *height,
            Self::Search { height, .. } => *height,
            Self::Password { height, .. } => *height,
        }
    }
}
