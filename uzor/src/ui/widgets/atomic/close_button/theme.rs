//! Close button theme trait and default implementation.

/// Color slots for the close button X glyph and hover background.
pub trait CloseButtonTheme {
    /// X glyph color in idle state.
    /// mlc watchlist idle: `item_text_muted`. Typical: `"#787b86"`.
    fn close_button_x_color(&self) -> &str;

    /// X glyph color when hovered.
    /// mlc watchlist hover: `item_text`. Typical: `"#ffffff"`.
    fn close_button_x_color_hover(&self) -> &str;

    /// Hover background fill color.
    /// Typical: `"#2a2e39"`.
    fn close_button_bg_hover(&self) -> &str;
}

/// Default close button theme.
pub struct DefaultCloseButtonTheme;

impl Default for DefaultCloseButtonTheme {
    fn default() -> Self {
        Self
    }
}

impl CloseButtonTheme for DefaultCloseButtonTheme {
    fn close_button_x_color(&self) -> &str       { "#787b86" }
    fn close_button_x_color_hover(&self) -> &str { "#ffffff" }
    fn close_button_bg_hover(&self) -> &str       { "#2a2e39" }
}
