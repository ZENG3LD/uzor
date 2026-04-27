//! Item widget theme trait and default implementation.

/// Color slots for item text and icon.
pub trait ItemTheme {
    /// Text / icon color for the item in its normal (non-interactive) state.
    /// Typical: `"#d1d4dc"`.
    fn item_text(&self) -> &str;

    /// Text / icon color for toolbar-style items.
    /// Typical: `"#d1d4dc"`.
    fn item_toolbar_text(&self) -> &str {
        self.item_text()
    }
}

/// Default item theme.
pub struct DefaultItemTheme;

impl Default for DefaultItemTheme {
    fn default() -> Self {
        Self
    }
}

impl ItemTheme for DefaultItemTheme {
    fn item_text(&self) -> &str { "#d1d4dc" }
}

/// Toolbar item theme — uses toolbar label text color.
pub struct ToolbarItemTheme;

impl ItemTheme for ToolbarItemTheme {
    fn item_text(&self) -> &str         { "#d1d4dc" }
    fn item_toolbar_text(&self) -> &str { "#d1d4dc" }
}
