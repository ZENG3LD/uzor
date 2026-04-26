//! Terminal styling types: Color, Modifier, Style.

use bitflags::bitflags;

/// Terminal color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum Color {
    #[default]
    Reset,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    /// Indexed color (0-255).
    Indexed(u8),
    /// RGB color.
    Rgb(u8, u8, u8),
}


bitflags! {
    /// Text modifiers as bitflags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifier: u16 {
        const BOLD          = 0b0000_0001;
        const DIM           = 0b0000_0010;
        const ITALIC        = 0b0000_0100;
        const UNDERLINE     = 0b0000_1000;
        const BLINK         = 0b0001_0000;
        const REVERSE       = 0b0100_0000;
        const HIDDEN        = 0b1000_0000;
        const STRIKETHROUGH = 0b0001_0000_0000;
    }
}

impl Default for Modifier {
    fn default() -> Self {
        Modifier::empty()
    }
}

/// Complete style for a terminal cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Style {
    pub fg: Color,
    pub bg: Color,
    pub modifiers: Modifier,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: Color::Reset,
            bg: Color::Reset,
            modifiers: Modifier::empty(),
        }
    }
}

impl Style {
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = color;
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg = color;
        self
    }

    pub fn add_modifier(mut self, modifier: Modifier) -> Self {
        self.modifiers |= modifier;
        self
    }
}
