//! Scroll chevron theme trait and default implementation.

/// Color slots for the scroll chevron glyph.
pub trait ScrollChevronTheme {
    /// Chevron color in idle state.
    /// Typical: `"#d1d4dc"`.
    fn scroll_chevron_color(&self) -> &str;

    /// Chevron color when hovered.
    /// Typical: `"#ffffff"`.
    fn scroll_chevron_color_hover(&self) -> &str;

    /// Chevron color when disabled (no items to scroll to).
    /// Typical: `"#4a4a4a"`.
    fn scroll_chevron_color_disabled(&self) -> &str;

    /// Hover background fill color.
    /// Typical: `"#2a2e39"`.
    fn scroll_chevron_bg_hover(&self) -> &str;
}

/// Default scroll chevron theme.
pub struct DefaultScrollChevronTheme;

impl Default for DefaultScrollChevronTheme {
    fn default() -> Self {
        Self
    }
}

impl ScrollChevronTheme for DefaultScrollChevronTheme {
    fn scroll_chevron_color(&self) -> &str          { "#d1d4dc" }
    fn scroll_chevron_color_hover(&self) -> &str    { "#ffffff" }
    fn scroll_chevron_color_disabled(&self) -> &str { "#4a4a4a" }
    fn scroll_chevron_bg_hover(&self) -> &str        { "#2a2e39" }
}
