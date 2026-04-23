//! Light appearance mode color palette

use super::ColorPalette;

pub const LIGHT: ColorPalette = ColorPalette {
    // Labels
    label: "#000000",
    secondary_label: "#3C3C43",
    tertiary_label: "#3C3C4399",
    quaternary_label: "#3C3C432E",

    // Text
    text: "#000000",
    placeholder_text: "#3C3C434C",
    selected_text: "#000000",
    text_background: "#FFFFFF",
    selected_text_background: "#B3D7FF",

    // Content
    link: "#0068DA",
    separator: "#0000001F",
    selected_content_background: "#0063E1",
    unemphasized_selected_content_background: "#E5E5E5",

    // Menu
    selected_menu_item_text: "#FFFFFF",

    // Table
    grid: "#D1D1D6",
    header_text: "#000000",
    alternating_even: "#FFFFFF",
    alternating_odd: "#F5F5F7",

    // Controls
    control_accent: "#007AFF",
    control: "#FFFFFF",
    control_background: "#FFFFFF",
    control_text: "#000000",
    disabled_control_text: "#3C3C434C",
    selected_control: "#007AFF",
    selected_control_text: "#FFFFFF",

    // Windows
    window_background: "#ECECEC",
    window_frame_text: "#000000",
    under_page_background: "#A8A8A8",

    // System accent colors
    system_blue: "#007AFF",
    system_brown: "#A2845E",
    system_gray: "#8E8E93",
    system_green: "#34C759",
    system_indigo: "#5856D6",
    system_orange: "#FF9500",
    system_pink: "#FF2D55",
    system_purple: "#AF52DE",
    system_red: "#FF3B30",
    system_teal: "#5AC8FA",
    system_yellow: "#FFCC00",

    // Fills
    fill_primary: "#00000014",
    fill_secondary: "#0000000F",
    fill_tertiary: "#0000000A",
    fill_quaternary: "#00000005",

    // Shadows
    shadow_color: "#00000040",
};
