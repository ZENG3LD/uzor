//! ColorSwatch theme trait — color contract for swatch rendering.

/// Theme trait for color swatch colors.
pub trait ColorSwatchTheme {
    /// Idle border drawn around a color swatch square.
    /// mlc: toolbar_theme.separator
    fn color_swatch_border(&self) -> &str;

    /// Outline drawn around the swatch when hovered or picker is open.
    /// mlc indicator_settings: toolbar_theme.item_bg_hover (expand rect fill color).
    fn color_swatch_hover_outline(&self) -> &str;

    /// Accent border color when the color picker is open (selected).
    /// mlc indicator_settings: toolbar_theme.accent
    fn color_swatch_selected_border(&self) -> &str;

    /// Light tile color for the transparency checkerboard background.
    /// mlc appearance tab: "#ffffff"
    fn transparency_checker_a(&self) -> &str;

    /// Dark tile color for the transparency checkerboard background.
    /// mlc appearance tab: "#cccccc"
    fn transparency_checker_b(&self) -> &str;

    /// Border color for the fill-toggle when fill is enabled (active state).
    /// mlc primitive_settings: toolbar_theme.item_bg_active
    fn fill_toggle_active_border(&self) -> &str;

    /// Diagonal strikethrough color for the fill-toggle when fill is disabled.
    /// mlc primitive_settings: toolbar_theme.separator
    fn fill_toggle_off_pattern_color(&self) -> &str;

    /// Background color used as the base for fill-toggle.
    /// mlc: toolbar_theme.background
    fn fill_toggle_background(&self) -> &str;
}

/// Default color swatch theme using prototype colors.
pub struct DefaultColorSwatchTheme;

impl DefaultColorSwatchTheme {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultColorSwatchTheme {
    fn default() -> Self {
        Self::new()
    }
}

impl ColorSwatchTheme for DefaultColorSwatchTheme {
    fn color_swatch_border(&self)          -> &str { "#2a2e39" }
    fn color_swatch_hover_outline(&self)   -> &str { "#2a2e39" }
    fn color_swatch_selected_border(&self) -> &str { "#2962ff" }
    fn transparency_checker_a(&self)       -> &str { "#ffffff" }
    fn transparency_checker_b(&self)       -> &str { "#cccccc" }
    fn fill_toggle_active_border(&self)    -> &str { "#2196F3" }
    fn fill_toggle_off_pattern_color(&self)-> &str { "#2a2e39" }
    fn fill_toggle_background(&self)       -> &str { "#1e222d" }
}
