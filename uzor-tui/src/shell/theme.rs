//! Color tokens for the default card skin. Apps can swap a theme.

use crate::style::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellTheme {
    pub modal: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub button_fg: Color,
    pub button_bg: Color,
    pub pending_bg: Color,
}

impl Default for ShellTheme {
    fn default() -> Self {
        Self {
            modal: Color::Black,
            border: Color::DarkGray,
            text: Color::White,
            muted: Color::DarkGray,
            accent: Color::Cyan,
            button_fg: Color::Black,
            button_bg: Color::Cyan,
            pending_bg: Color::Yellow,
        }
    }
}
