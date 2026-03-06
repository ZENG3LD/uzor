//! macOS semantic color system with 8 appearance modes

mod light;
mod dark;
pub mod helpers;

pub use light::LIGHT;
pub use dark::DARK;
pub use helpers::color_with_alpha;

/// macOS appearance mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum AppearanceMode {
    Light,
    #[default]
    Dark,
    VibrantLight,
    VibrantDark,
    AccessibleLight,
    AccessibleDark,
    AccessibleVibrantLight,
    AccessibleVibrantDark,
}


/// Widget interaction state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[derive(Default)]
pub enum WidgetState {
    #[default]
    Normal,
    Hovered,
    Pressed,
    Disabled,
    Focused,
}


/// A complete color palette for one appearance mode
pub struct ColorPalette {
    // Labels
    pub label: &'static str,
    pub secondary_label: &'static str,
    pub tertiary_label: &'static str,
    pub quaternary_label: &'static str,

    // Text
    pub text: &'static str,
    pub placeholder_text: &'static str,
    pub selected_text: &'static str,
    pub text_background: &'static str,
    pub selected_text_background: &'static str,

    // Content
    pub link: &'static str,
    pub separator: &'static str,
    pub selected_content_background: &'static str,
    pub unemphasized_selected_content_background: &'static str,

    // Menu
    pub selected_menu_item_text: &'static str,

    // Table
    pub grid: &'static str,
    pub header_text: &'static str,
    pub alternating_even: &'static str,
    pub alternating_odd: &'static str,

    // Controls
    pub control_accent: &'static str,
    pub control: &'static str,
    pub control_background: &'static str,
    pub control_text: &'static str,
    pub disabled_control_text: &'static str,
    pub selected_control: &'static str,
    pub selected_control_text: &'static str,

    // Windows
    pub window_background: &'static str,
    pub window_frame_text: &'static str,
    pub under_page_background: &'static str,

    // System accent colors
    pub system_blue: &'static str,
    pub system_brown: &'static str,
    pub system_gray: &'static str,
    pub system_green: &'static str,
    pub system_indigo: &'static str,
    pub system_orange: &'static str,
    pub system_pink: &'static str,
    pub system_purple: &'static str,
    pub system_red: &'static str,
    pub system_teal: &'static str,
    pub system_yellow: &'static str,

    // Fills (with opacity)
    pub fill_primary: &'static str,
    pub fill_secondary: &'static str,
    pub fill_tertiary: &'static str,
    pub fill_quaternary: &'static str,

    // Shadows
    pub shadow_color: &'static str,
}

/// Resolve a palette for the given appearance mode
pub fn palette(mode: AppearanceMode) -> &'static ColorPalette {
    match mode {
        AppearanceMode::Dark | AppearanceMode::VibrantDark
        | AppearanceMode::AccessibleDark | AppearanceMode::AccessibleVibrantDark => &DARK,
        _ => &LIGHT,
    }
}
