//! Checkbox theme trait and default implementation.

/// Color contract for checkbox rendering.
pub trait CheckboxTheme {
    /// Checkbox fill when checked (active state).
    fn checkbox_bg_checked(&self) -> &str;
    /// Checkbox fill when unchecked.
    fn checkbox_bg_unchecked(&self) -> &str;
    /// Checkbox border color.
    fn checkbox_border(&self) -> &str;
    /// Checkmark stroke color (the ✓ path).
    fn checkbox_checkmark(&self) -> &str;
    /// Notification-checkbox inner fill color when enabled.
    fn checkbox_notification_inner(&self) -> &str;
    /// Label text color.
    fn checkbox_label_text(&self) -> &str;
}

/// Default checkbox theme using uzor prototype colors.
pub struct DefaultCheckboxTheme;

impl Default for DefaultCheckboxTheme {
    fn default() -> Self {
        Self
    }
}

impl CheckboxTheme for DefaultCheckboxTheme {
    fn checkbox_bg_checked(&self) -> &str          { "#2196F3" }
    fn checkbox_bg_unchecked(&self) -> &str        { "#1e222d" }
    fn checkbox_border(&self) -> &str              { "#2a2e39" }
    fn checkbox_checkmark(&self) -> &str           { "#ffffff" }
    fn checkbox_notification_inner(&self) -> &str  { "#ffffff" }
    fn checkbox_label_text(&self) -> &str          { "#d1d4dc" }
}
