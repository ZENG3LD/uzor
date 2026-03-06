//! macOS Ventura complete theme preset
//!
//! Provides a one-stop shop for all macOS-styled themes in a single struct.

use crate::colors::{AppearanceMode, ColorPalette, palette};
use crate::themes::button::{ButtonTheme, ButtonVariant};
use crate::themes::menu::MenuTheme;
use crate::themes::checkbox::CheckboxTheme;
use crate::themes::radio::RadioTheme;
use crate::themes::switch_toggle::SwitchTheme;
use crate::themes::input::InputTheme;
use crate::themes::dialog::DialogTheme;
use crate::themes::traffic_lights::TrafficLightTheme;
use crate::themes::progress::ProgressTheme;
use crate::themes::tabs::TabTheme;

/// macOS Ventura theme preset
///
/// # Usage
/// ```
/// use uzor_macos::presets::ventura::VenturaPreset;
/// use uzor_macos::colors::AppearanceMode;
///
/// let preset = VenturaPreset::new(AppearanceMode::Dark);
/// let button_theme = preset.button_theme();
/// let menu_theme = preset.menu_theme();
/// ```
pub struct VenturaPreset {
    mode: AppearanceMode,
}

impl VenturaPreset {
    pub fn new(mode: AppearanceMode) -> Self {
        Self { mode }
    }

    pub fn dark() -> Self {
        Self::new(AppearanceMode::Dark)
    }

    pub fn light() -> Self {
        Self::new(AppearanceMode::Light)
    }

    pub fn mode(&self) -> AppearanceMode {
        self.mode
    }

    pub fn colors(&self) -> &'static ColorPalette {
        palette(self.mode)
    }

    // Return theme objects for each widget type

    /// Default button theme
    pub fn button_theme(&self) -> ButtonTheme {
        ButtonTheme::new(ButtonVariant::Default, self.mode)
    }

    /// Accent (blue) button theme
    pub fn accent_button_theme(&self) -> ButtonTheme {
        ButtonTheme::new(ButtonVariant::Accent, self.mode)
    }

    /// Destructive (red) button theme
    pub fn destructive_button_theme(&self) -> ButtonTheme {
        ButtonTheme::new(ButtonVariant::Destructive, self.mode)
    }

    /// Menu theme
    pub fn menu_theme(&self) -> MenuTheme {
        MenuTheme::new(self.mode)
    }

    /// Checkbox theme
    pub fn checkbox_theme(&self) -> CheckboxTheme {
        CheckboxTheme::new(self.mode)
    }

    /// Radio button theme
    pub fn radio_theme(&self) -> RadioTheme {
        RadioTheme::new(self.mode)
    }

    /// Switch toggle theme
    pub fn switch_theme(&self) -> SwitchTheme {
        SwitchTheme::new(self.mode)
    }

    /// Input field theme
    pub fn input_theme(&self) -> InputTheme {
        InputTheme::new(self.mode)
    }

    /// Dialog theme
    pub fn dialog_theme(&self) -> DialogTheme {
        DialogTheme::new(self.mode)
    }

    /// Traffic lights theme
    pub fn traffic_light_theme(&self) -> TrafficLightTheme {
        TrafficLightTheme::new(self.mode)
    }

    /// Progress bar/ring theme
    pub fn progress_theme(&self) -> ProgressTheme {
        ProgressTheme::new(self.mode)
    }

    /// Tab theme
    pub fn tab_theme(&self) -> TabTheme {
        TabTheme::new(self.mode)
    }
}

impl Default for VenturaPreset {
    fn default() -> Self {
        Self::new(AppearanceMode::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ventura_preset_new() {
        let preset = VenturaPreset::new(AppearanceMode::Dark);
        assert_eq!(preset.mode(), AppearanceMode::Dark);
    }

    #[test]
    fn test_ventura_preset_dark() {
        let preset = VenturaPreset::dark();
        assert_eq!(preset.mode(), AppearanceMode::Dark);
    }

    #[test]
    fn test_ventura_preset_light() {
        let preset = VenturaPreset::light();
        assert_eq!(preset.mode(), AppearanceMode::Light);
    }

    #[test]
    fn test_ventura_preset_default() {
        let preset = VenturaPreset::default();
        assert_eq!(preset.mode(), AppearanceMode::default());
    }

    #[test]
    fn test_ventura_preset_colors() {
        let preset = VenturaPreset::dark();
        let colors = preset.colors();
        assert_eq!(colors.label, "#FFFFFF");
    }

    #[test]
    fn test_ventura_preset_button_theme() {
        let preset = VenturaPreset::dark();
        let theme = preset.button_theme();
        assert_eq!(theme.variant, ButtonVariant::Default);
    }

    #[test]
    fn test_ventura_preset_accent_button_theme() {
        let preset = VenturaPreset::light();
        let theme = preset.accent_button_theme();
        assert_eq!(theme.variant, ButtonVariant::Accent);
    }

    #[test]
    fn test_ventura_preset_all_themes() {
        let preset = VenturaPreset::dark();

        // Ensure all theme getters work
        let _button = preset.button_theme();
        let _accent = preset.accent_button_theme();
        let _destructive = preset.destructive_button_theme();
        let _menu = preset.menu_theme();
        let _checkbox = preset.checkbox_theme();
        let _radio = preset.radio_theme();
        let _switch = preset.switch_theme();
        let _input = preset.input_theme();
        let _dialog = preset.dialog_theme();
        let _traffic_light = preset.traffic_light_theme();
        let _progress = preset.progress_theme();
        let _tab = preset.tab_theme();
    }
}
