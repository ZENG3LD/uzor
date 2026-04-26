//! Text input widget — visual variant + capability declaration.
//!
//! Layout (position/size) belongs in the layout layer, not in widget
//! data. Renderer takes the rect as a parameter and reads other params
//! from `TextInputSettings`.

use crate::input::Sense;
use crate::ui::widgets::WidgetCapabilities;

/// Visual variant. Affects placeholder semantics, password masking,
/// and (later) inline icons (e.g. magnifier for `Search`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputType {
    #[default]
    Text,
    Number,
    Search,
    Password,
}

/// Top-level widget marker (kept for back-compat with existing call sites).
/// Most callers should pass `InputType` directly; this enum carries no
/// extra data over `InputType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputType {
    Text,
    Number,
    Search,
    Password,
}

impl From<TextInputType> for InputType {
    fn from(t: TextInputType) -> InputType {
        match t {
            TextInputType::Text     => InputType::Text,
            TextInputType::Number   => InputType::Number,
            TextInputType::Search   => InputType::Search,
            TextInputType::Password => InputType::Password,
        }
    }
}

impl InputType {
    /// Mask `value` for display when the variant requires it (Password).
    /// Other variants return `value` unchanged.
    pub fn display(&self, value: &str) -> String {
        match self {
            InputType::Password => "•".repeat(value.chars().count()),
            _                   => value.to_string(),
        }
    }
}

impl WidgetCapabilities for TextInputType {
    fn sense(&self) -> Sense {
        Sense::CLICK.with_focus().with_text()
    }
}
