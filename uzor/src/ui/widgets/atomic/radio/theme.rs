//! Radio theme trait and default implementation.

/// Color contract for radio rendering.
pub trait RadioTheme {
    /// Stroke color for the outer ring of an unselected radio button.
    fn radio_outer_border(&self) -> &str;
    /// Stroke color for the outer ring of a selected radio button.
    fn radio_outer_border_selected(&self) -> &str;
    /// Fill color for the inner dot of a selected radio button.
    fn radio_inner_dot(&self) -> &str;
    /// Overlay applied over a disabled radio.
    fn radio_disabled_overlay(&self) -> &str;
    /// Hover-row background for `Group` variant.
    fn radio_row_bg_hover(&self) -> &str;
    /// Normal label text color.
    fn radio_label_text(&self) -> &str;
    /// Label text color when the row is selected.
    fn radio_label_text_selected(&self) -> &str;
    /// Muted description text color.
    fn radio_description_text(&self) -> &str;
}

/// Default radio theme using uzor prototype colors.
pub struct DefaultRadioTheme;

impl Default for DefaultRadioTheme {
    fn default() -> Self {
        Self
    }
}

impl RadioTheme for DefaultRadioTheme {
    fn radio_outer_border(&self) -> &str           { "#2a2e39" }
    fn radio_outer_border_selected(&self) -> &str  { "#2962ff" }
    fn radio_inner_dot(&self) -> &str              { "#2962ff" }
    fn radio_disabled_overlay(&self) -> &str       { "rgba(0,0,0,0.35)" }
    fn radio_row_bg_hover(&self) -> &str           { "#2a2e39" }
    fn radio_label_text(&self) -> &str             { "#d1d4dc" }
    fn radio_label_text_selected(&self) -> &str    { "#ffffff" }
    fn radio_description_text(&self) -> &str       { "#4a4a4a" }
}
