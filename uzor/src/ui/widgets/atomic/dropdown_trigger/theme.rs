//! DropdownTrigger theme trait — color contract for trigger rendering.

/// Theme trait for dropdown trigger colors.
pub trait DropdownTriggerTheme {
    /// Background fill for a trigger in idle state.
    /// mlc alert_settings: toolbar_theme.dropdown_bg (≈ toolbar_background).
    fn dropdown_field_bg(&self) -> &str;

    /// Background fill for a trigger on hover or when open.
    /// mlc alert_settings: toolbar_theme.item_bg_hover.
    fn dropdown_field_bg_hover(&self) -> &str;

    /// Border color for a trigger.
    /// mlc: toolbar_theme.separator.
    fn dropdown_field_border(&self) -> &str;

    /// Text color inside a trigger.
    fn dropdown_field_text(&self) -> &str;

    /// Chevron icon color used in triggers.
    /// mlc: toolbar_theme.item_text (or item_text_muted).
    fn dropdown_chevron_color(&self) -> &str;
}

/// Default dropdown trigger theme using prototype colors.
pub struct DefaultDropdownTriggerTheme;

impl DefaultDropdownTriggerTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultDropdownTriggerTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl DropdownTriggerTheme for DefaultDropdownTriggerTheme {
    fn dropdown_field_bg(&self)       -> &str { "#1e222d" }
    fn dropdown_field_bg_hover(&self) -> &str { "#2a2e39" }
    fn dropdown_field_border(&self)   -> &str { "#2a2e39" }
    fn dropdown_field_text(&self)     -> &str { "#d1d4dc" }
    fn dropdown_chevron_color(&self)  -> &str { "#d1d4dc" }
}
