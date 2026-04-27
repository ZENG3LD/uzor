//! ShapeSelector theme trait — color contract for selector rendering.

/// Theme trait for shape selector / preset selector colors.
pub trait ShapeSelectorTheme {
    /// Border for a selector button in idle (not selected, not hovered) state.
    /// mlc indicator_settings shape selector: toolbar_theme.separator
    fn selector_idle_border(&self) -> &str;

    /// Border drawn around the selected selector button.
    /// mlc shape selector active: toolbar_theme.accent
    fn selector_selected_border(&self) -> &str;

    /// Border drawn when the pointer is over an unselected selector button.
    /// mlc shape selector hover: toolbar_theme.item_bg_hover (used as bg, no extra outline)
    fn selector_hover_border(&self) -> &str;

    /// Label text color below / beside the selector button.
    fn selector_label_text(&self) -> &str;

    /// Background when the button is idle (unselected, not hovered).
    fn selector_idle_bg(&self) -> &str;

    /// Background when the button is hovered.
    fn selector_hover_bg(&self) -> &str;

    /// Background when the button is selected / active.
    fn selector_active_bg(&self) -> &str;

    /// Text color for selected button content.
    fn selector_active_text(&self) -> &str;

    /// Text color for idle button content.
    fn selector_idle_text(&self) -> &str;
}

/// Default shape selector theme using prototype colors.
pub struct DefaultShapeSelectorTheme;

impl DefaultShapeSelectorTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultShapeSelectorTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl ShapeSelectorTheme for DefaultShapeSelectorTheme {
    fn selector_idle_border(&self)     -> &str { "#2a2e39" }
    fn selector_selected_border(&self) -> &str { "#2962ff" }
    fn selector_hover_border(&self)    -> &str { "#2a2e39" }
    fn selector_label_text(&self)      -> &str { "#d1d4dc" }
    fn selector_idle_bg(&self)         -> &str { "#1e222d" }
    fn selector_hover_bg(&self)        -> &str { "#2a2e39" }
    fn selector_active_bg(&self)       -> &str { "#2196F3" }
    fn selector_active_text(&self)     -> &str { "#ffffff" }
    fn selector_idle_text(&self)       -> &str { "#d1d4dc" }
}
