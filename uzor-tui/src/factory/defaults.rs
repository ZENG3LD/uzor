//! TUI rendering defaults - sizes, colors, icons

use crate::style::Color;

/// Default sizes and spacing for button rendering
#[derive(Debug, Clone)]
pub struct TuiButtonDefaults {
    pub icon_only_width: u16,       // 3 cells [X]
    pub text_padding: u16,          // 1 cell each side
    pub min_text_width: u16,        // 5 cells
    pub checkbox_width: u16,        // 3 cells [✓]
}

impl Default for TuiButtonDefaults {
    fn default() -> Self {
        Self {
            icon_only_width: 3,
            text_padding: 1,
            min_text_width: 5,
            checkbox_width: 3,
        }
    }
}

/// TUI color palette for widgets
#[derive(Debug, Clone)]
pub struct TuiColors {
    // Background colors
    pub bg_normal: Color,           // Color::Reset
    pub bg_hover: Color,            // Color::Indexed(238)
    pub bg_pressed: Color,          // Color::Indexed(240)
    pub bg_active: Color,           // Color::Indexed(236)
    pub bg_disabled: Color,         // Color::Indexed(234)

    // Foreground colors
    pub fg_normal: Color,           // Color::White
    pub fg_dimmed: Color,           // Color::Indexed(245)
    pub fg_disabled: Color,         // Color::Indexed(242)
    pub fg_accent: Color,           // Color::Green
    pub fg_danger: Color,           // Color::Red
    pub fg_warning: Color,          // Color::Yellow
    pub fg_success: Color,          // Color::Green
    pub fg_info: Color,             // Color::Blue

    // Border colors
    pub border_normal: Color,       // Color::Indexed(240)
    pub border_active: Color,       // Color::Green
    pub border_focused: Color,      // Color::Blue
}

impl Default for TuiColors {
    fn default() -> Self {
        Self {
            bg_normal: Color::Reset,
            bg_hover: Color::Indexed(238),
            bg_pressed: Color::Indexed(240),
            bg_active: Color::Indexed(236),
            bg_disabled: Color::Indexed(234),

            fg_normal: Color::White,
            fg_dimmed: Color::Indexed(245),
            fg_disabled: Color::Indexed(242),
            fg_accent: Color::Green,
            fg_danger: Color::Red,
            fg_warning: Color::Yellow,
            fg_success: Color::Green,
            fg_info: Color::Blue,

            border_normal: Color::Indexed(240),
            border_active: Color::Green,
            border_focused: Color::Blue,
        }
    }
}

/// Unicode icons for TUI rendering
pub struct TuiIcons;

impl TuiIcons {
    pub const CLOSE: &'static str = "×";
    pub const ADD: &'static str = "+";
    pub const CHECK: &'static str = "✓";
    pub const CROSS: &'static str = "✗";
    pub const CHEVRON_DOWN: &'static str = "▼";
    pub const CHEVRON_RIGHT: &'static str = "▶";
    pub const CHEVRON_LEFT: &'static str = "◀";
    pub const CHEVRON_UP: &'static str = "▲";
    pub const CIRCLE_FILLED: &'static str = "●";
    pub const CIRCLE_EMPTY: &'static str = "○";
    pub const INFO: &'static str = "ℹ";
    pub const WARNING: &'static str = "⚠";
    pub const SLIDER_FILL: &'static str = "█";
    pub const SLIDER_EMPTY: &'static str = "░";
    pub const SCROLLBAR_TRACK: &'static str = "║";
    pub const SCROLLBAR_THUMB: &'static str = "█";
}

/// Map IconId to Unicode character for TUI rendering
pub fn icon_to_char(icon: &uzor_core::types::IconId) -> &'static str {
    match icon.name() {
        "close" | "x" => TuiIcons::CLOSE,
        "add" | "plus" => TuiIcons::ADD,
        "check" | "checkmark" => TuiIcons::CHECK,
        "search" => "🔍",
        "settings" | "gear" => "⚙",
        "eye" => "👁",
        "eye_off" | "eye-off" => "⊗",
        "chevron_down" | "chevron-down" => TuiIcons::CHEVRON_DOWN,
        "chevron_right" | "chevron-right" => TuiIcons::CHEVRON_RIGHT,
        "chevron_left" | "chevron-left" => TuiIcons::CHEVRON_LEFT,
        "chevron_up" | "chevron-up" => TuiIcons::CHEVRON_UP,
        "trash" | "delete" => "🗑",
        "lock" => "🔒",
        "unlock" => "🔓",
        "alert" | "bell" => "🔔",
        "more" | "dots" => "•••",
        _ => "•",
    }
}
