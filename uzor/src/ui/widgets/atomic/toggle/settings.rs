//! Bundled per-instance settings for the toggle widget.

use super::theme::{DefaultToggleTheme, ToggleTheme};
use super::style::{DefaultToggleIconStyle, IndicatorToggleStyle, ToggleIconStyle, ToggleSwitchStyle};

/// Aggregates visual configuration for a toggle instance.
pub struct ToggleSettings {
    /// Color slots.
    pub theme: Box<dyn ToggleTheme>,
    /// Track/thumb geometry for `Switch` / `SwitchWide` variants.
    pub switch_style: Box<dyn ToggleSwitchStyle>,
    /// Icon geometry for `IconSwap` variant.
    pub icon_style: Box<dyn ToggleIconStyle>,
}

impl Default for ToggleSettings {
    fn default() -> Self {
        Self {
            theme: Box::new(DefaultToggleTheme),
            switch_style: Box::new(IndicatorToggleStyle),
            icon_style: Box::new(DefaultToggleIconStyle),
        }
    }
}

impl ToggleSettings {
    pub fn with_theme(mut self, theme: Box<dyn ToggleTheme>) -> Self {
        self.theme = theme;
        self
    }

    pub fn with_switch_style(mut self, style: Box<dyn ToggleSwitchStyle>) -> Self {
        self.switch_style = style;
        self
    }

    pub fn with_icon_style(mut self, style: Box<dyn ToggleIconStyle>) -> Self {
        self.icon_style = style;
        self
    }
}
