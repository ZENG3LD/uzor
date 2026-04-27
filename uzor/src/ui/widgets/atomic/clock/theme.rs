//! Clock widget theme trait and default implementation.

/// Color slots for the clock text and hover background.
pub trait ClockTheme {
    /// Time text color.
    /// Typical: `"#d1d4dc"`.
    fn clock_text(&self) -> &str;

    /// Hover background fill color (drawn with vertical inset).
    /// Typical: `"#2a2e39"`.
    fn clock_bg_hover(&self) -> &str;
}

/// Default clock theme.
pub struct DefaultClockTheme;

impl Default for DefaultClockTheme {
    fn default() -> Self {
        Self
    }
}

impl ClockTheme for DefaultClockTheme {
    fn clock_text(&self) -> &str    { "#d1d4dc" }
    fn clock_bg_hover(&self) -> &str { "#2a2e39" }
}
