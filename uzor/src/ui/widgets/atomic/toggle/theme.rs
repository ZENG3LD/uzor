//! Toggle theme trait and default implementation.

/// Color contract for toggle rendering.
pub trait ToggleTheme {
    /// Track fill when OFF. mlc: `toolbar_item_bg_hover`.
    fn toggle_track_off(&self) -> &str;
    /// Track fill when ON. mlc: `toolbar_accent`.
    fn toggle_track_on(&self) -> &str;
    /// Thumb fill when OFF (white in mlc).
    fn toggle_thumb_off(&self) -> &str;
    /// Thumb fill when ON (white in mlc).
    fn toggle_thumb_on(&self) -> &str;
    /// Overlay applied over the whole toggle when disabled.
    fn toggle_disabled_overlay(&self) -> &str;
    /// Normal label text color.
    fn toggle_label_text(&self) -> &str;
    /// Disabled label text color.
    fn toggle_label_text_disabled(&self) -> &str;
    /// Icon color for the IconSwap variant in normal state.
    fn toggle_icon_normal(&self) -> &str;
    /// Icon color for the IconSwap variant when ON / active.
    fn toggle_icon_active(&self) -> &str;
}

/// Default toggle theme using uzor prototype colors.
pub struct DefaultToggleTheme;

impl Default for DefaultToggleTheme {
    fn default() -> Self {
        Self
    }
}

impl ToggleTheme for DefaultToggleTheme {
    fn toggle_track_off(&self) -> &str          { "#2a2e39" }
    fn toggle_track_on(&self) -> &str           { "#2962ff" }
    fn toggle_thumb_off(&self) -> &str          { "#ffffff" }
    fn toggle_thumb_on(&self) -> &str           { "#ffffff" }
    fn toggle_disabled_overlay(&self) -> &str   { "rgba(0,0,0,0.35)" }
    fn toggle_label_text(&self) -> &str         { "#d1d4dc" }
    fn toggle_label_text_disabled(&self) -> &str{ "#4a4a4a" }
    fn toggle_icon_normal(&self) -> &str        { "#787b86" }
    fn toggle_icon_active(&self) -> &str        { "#ffffff" }
}
