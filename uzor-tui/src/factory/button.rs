//! Button rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor::widgets::button::types::*;
use uzor::types::WidgetState;
use super::defaults::*;

/// Render a button to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    button: &ButtonType,
    area: Rect,
    state: WidgetState,
) {
    let colors = TuiColors::default();

    let bg = match state {
        WidgetState::Pressed => colors.bg_pressed,
        WidgetState::Hovered => colors.bg_hover,
        WidgetState::Active | WidgetState::Toggled => colors.bg_active,
        WidgetState::Disabled => colors.bg_disabled,
        WidgetState::Normal => colors.bg_normal,
    };

    match button {
        ButtonType::Action { variant, .. } => {
            render_action(buf, variant, area, state, bg, &colors);
        }
        ButtonType::Toggle { variant, .. } => {
            render_toggle(buf, variant, area, state, bg, &colors);
        }
        ButtonType::Checkbox { variant, .. } => {
            render_checkbox(buf, variant, area, state, bg, &colors);
        }
        ButtonType::Tab { variant, .. } => {
            render_tab(buf, variant, area, state, bg, &colors);
        }
        ButtonType::ColorSwatch { variant, .. } => {
            render_color_swatch(buf, variant, area, state, bg, &colors);
        }
        ButtonType::Dropdown { variant, .. } => {
            render_dropdown(buf, variant, area, state, bg, &colors);
        }
    }
}

fn render_action(
    buf: &mut TerminalBuffer,
    variant: &ActionVariant,
    area: Rect,
    _state: WidgetState,
    bg: Color,
    colors: &TuiColors,
) {
    match variant {
        ActionVariant::IconOnly { icon, disabled } => {
            let icon_char = icon_to_char(icon);
            let fg = if *disabled { colors.fg_disabled } else { colors.fg_normal };
            let style = Style { fg, bg, modifiers: Modifier::empty() };

            // Render: [icon]
            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, icon_char, style);
            set_string(buf, area.x + 2, area.y, "]", style);
        }
        ActionVariant::Text { text, disabled, .. } => {
            let fg = if *disabled { colors.fg_disabled } else { colors.fg_normal };
            let style = Style { fg, bg, modifiers: Modifier::empty() };
            // Render: [ Text ]
            let display = format!(" {} ", text);
            set_string(buf, area.x, area.y, &display, style);
        }
        ActionVariant::IconText { icon, text, disabled, .. } => {
            let icon_char = icon_to_char(icon);
            let fg = if *disabled { colors.fg_disabled } else { colors.fg_normal };
            let style = Style { fg, bg, modifiers: Modifier::empty() };
            let display = format!("{} {}", icon_char, text);
            set_string(buf, area.x, area.y, &display, style);
        }
        ActionVariant::LineText { line_width, text, disabled, .. } => {
            let fg = if *disabled { colors.fg_disabled } else { colors.fg_normal };
            let style = Style { fg, bg, modifiers: Modifier::empty() };
            let line_char = match line_width {
                1 => "─",
                2 => "━",
                _ => "━",
            };
            let display = format!("{}{}{} {}", line_char, line_char, line_char, text);
            set_string(buf, area.x, area.y, &display, style);
        }
        ActionVariant::CheckboxText { checkbox_checked, text, disabled, .. } => {
            let fg = if *disabled { colors.fg_disabled } else { colors.fg_normal };
            let check_fg = if *checkbox_checked { colors.fg_accent } else { colors.fg_dimmed };
            let bg_style = Style { fg, bg, modifiers: Modifier::empty() };
            let check_style = Style { fg: check_fg, bg, modifiers: Modifier::empty() };

            let check_symbol = if *checkbox_checked { TuiIcons::CHECK } else { " " };
            set_string(buf, area.x, area.y, "[", check_style);
            set_string(buf, area.x + 1, area.y, check_symbol, check_style);
            set_string(buf, area.x + 2, area.y, "]", check_style);
            set_string(buf, area.x + 3, area.y, " ", bg_style);
            set_string(buf, area.x + 4, area.y, text, bg_style);
        }
    }
}

fn render_toggle(
    buf: &mut TerminalBuffer,
    variant: &ToggleVariant,
    area: Rect,
    _state: WidgetState,
    bg: Color,
    colors: &TuiColors,
) {
    match variant {
        ToggleVariant::IconSwap { icon_off, icon_on, toggled } => {
            let icon = if *toggled { icon_on } else { icon_off };
            let icon_char = icon_to_char(icon);
            let fg = if *toggled { colors.fg_accent } else { colors.fg_normal };
            let style = Style { fg, bg, modifiers: Modifier::empty() };

            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, icon_char, style);
            set_string(buf, area.x + 2, area.y, "]", style);
        }
        ToggleVariant::Switch { toggled, label } => {
            let fg = if *toggled { colors.fg_accent } else { colors.fg_dimmed };
            let style = Style { fg, bg, modifiers: Modifier::empty() };

            // iOS-style: [○━━] or [━━●]
            if *toggled {
                set_string(buf, area.x, area.y, "[━━", style);
                set_string(buf, area.x + 3, area.y, TuiIcons::CIRCLE_FILLED, Style { fg: colors.fg_accent, bg, modifiers: Modifier::empty() });
                set_string(buf, area.x + 4, area.y, "]", style);
            } else {
                set_string(buf, area.x, area.y, "[", style);
                set_string(buf, area.x + 1, area.y, TuiIcons::CIRCLE_EMPTY, style);
                set_string(buf, area.x + 2, area.y, "━━]", style);
            }

            if let Some(label_text) = label {
                let label_style = Style { fg: colors.fg_normal, bg, modifiers: Modifier::empty() };
                set_string(buf, area.x + 6, area.y, label_text, label_style);
            }
        }
        ToggleVariant::ButtonToggle { content, toggled, .. } => {
            let fg = if *toggled { colors.fg_accent } else { colors.fg_normal };
            let bg_button = if *toggled { colors.bg_active } else { bg };
            let style = Style { fg, bg: bg_button, modifiers: Modifier::empty() };

            let mut x = area.x;
            if let Some(icon) = &content.icon {
                let icon_char = icon_to_char(icon);
                set_string(buf, x, area.y, icon_char, style);
                x += 2;
            }
            if let Some(text) = &content.text {
                set_string(buf, x, area.y, text, style);
            }
        }
    }
}

fn render_checkbox(
    buf: &mut TerminalBuffer,
    variant: &CheckboxVariant,
    area: Rect,
    _state: WidgetState,
    bg: Color,
    colors: &TuiColors,
) {
    match variant {
        CheckboxVariant::Standard { checked } => {
            let symbol = if *checked { TuiIcons::CHECK } else { " " };
            let fg = if *checked { colors.fg_accent } else { colors.fg_dimmed };
            let style = Style { fg, bg, modifiers: Modifier::empty() };

            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, symbol, style);
            set_string(buf, area.x + 2, area.y, "]", style);
        }
        CheckboxVariant::Cross { checked } => {
            let symbol = if *checked { TuiIcons::CROSS } else { " " };
            let fg = if *checked { colors.fg_danger } else { colors.fg_dimmed };
            let style = Style { fg, bg, modifiers: Modifier::empty() };

            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, symbol, style);
            set_string(buf, area.x + 2, area.y, "]", style);
        }
        CheckboxVariant::Circle { checked } => {
            let symbol = if *checked { TuiIcons::CIRCLE_FILLED } else { TuiIcons::CIRCLE_EMPTY };
            let fg = if *checked { colors.fg_accent } else { colors.fg_dimmed };
            let style = Style { fg, bg, modifiers: Modifier::empty() };

            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, symbol, style);
            set_string(buf, area.x + 2, area.y, "]", style);
        }
    }
}

fn render_tab(
    buf: &mut TerminalBuffer,
    variant: &TabVariant,
    area: Rect,
    _state: WidgetState,
    bg: Color,
    colors: &TuiColors,
) {
    match variant {
        TabVariant::Vertical { label, icon, active } => {
            let fg = if *active { colors.fg_accent } else { colors.fg_normal };
            let bg_tab = if *active { colors.bg_active } else { bg };
            let style = Style { fg, bg: bg_tab, modifiers: Modifier::empty() };

            let mut x = area.x;

            // Active indicator: "│ "
            if *active {
                let indicator_style = Style { fg: colors.border_active, bg: bg_tab, modifiers: Modifier::empty() };
                set_string(buf, x, area.y, "│", indicator_style);
                x += 1;
            } else {
                set_string(buf, x, area.y, " ", style);
                x += 1;
            }

            if let Some(icon_id) = icon {
                let icon_char = icon_to_char(icon_id);
                set_string(buf, x, area.y, icon_char, style);
                x += 2;
            }

            if let Some(label_text) = label {
                set_string(buf, x, area.y, label_text, style);
            }
        }
        TabVariant::Horizontal { label, icon, active } => {
            let fg = if *active { colors.fg_accent } else { colors.fg_normal };
            let bg_tab = if *active { colors.bg_active } else { bg };
            let style = Style { fg, bg: bg_tab, modifiers: Modifier::empty() };

            let mut x = area.x;

            if let Some(icon_id) = icon {
                let icon_char = icon_to_char(icon_id);
                set_string(buf, x, area.y, icon_char, style);
                x += 2;
            }

            if let Some(label_text) = label {
                set_string(buf, x, area.y, label_text, style);
            }

            // Underline for active tab
            if *active && area.height > 1 {
                let underline_y = area.y + 1;
                let underline_style = Style { fg: colors.border_active, bg, modifiers: Modifier::empty() };
                for col in area.x..area.x + area.width {
                    set_string(buf, col, underline_y, "─", underline_style);
                }
            }
        }
    }
}

fn render_color_swatch(
    buf: &mut TerminalBuffer,
    variant: &ColorSwatchVariant,
    area: Rect,
    _state: WidgetState,
    _bg: Color,
    colors: &TuiColors,
) {
    match variant {
        ColorSwatchVariant::Square { color } => {
            // Parse hex color or use indexed color
            let swatch_color = parse_color(color);
            let swatch_style = Style { fg: Color::White, bg: swatch_color, modifiers: Modifier::empty() };

            // Render colored square: ███
            for i in 0..3 {
                set_string(buf, area.x + i, area.y, "█", swatch_style);
            }
        }
        ColorSwatchVariant::IconWithBar { icon, color } => {
            let icon_char = icon_to_char(icon);
            let icon_style = Style { fg: colors.fg_normal, bg: Color::Reset, modifiers: Modifier::empty() };

            // Icon on first row
            set_string(buf, area.x, area.y, icon_char, icon_style);

            // Color bar below if space
            if area.height > 1 {
                let swatch_color = parse_color(color);
                let bar_style = Style { fg: Color::White, bg: swatch_color, modifiers: Modifier::empty() };
                set_string(buf, area.x, area.y + 1, "█", bar_style);
            }
        }
        ColorSwatchVariant::SwatchWithLabel { color, label } => {
            let swatch_color = parse_color(color);
            let swatch_style = Style { fg: Color::White, bg: swatch_color, modifiers: Modifier::empty() };
            let label_style = Style { fg: colors.fg_normal, bg: Color::Reset, modifiers: Modifier::empty() };

            // Colored square
            for i in 0..2 {
                set_string(buf, area.x + i, area.y, "█", swatch_style);
            }

            // Label
            set_string(buf, area.x + 3, area.y, label, label_style);
        }
    }
}

fn render_dropdown(
    buf: &mut TerminalBuffer,
    variant: &DropdownVariant,
    area: Rect,
    _state: WidgetState,
    bg: Color,
    colors: &TuiColors,
) {
    let style = Style { fg: colors.fg_normal, bg, modifiers: Modifier::empty() };
    let chevron_style = Style { fg: colors.fg_dimmed, bg, modifiers: Modifier::empty() };

    match variant {
        DropdownVariant::TextChevron { current_label, .. } => {
            // Render: [Text.................|▼]
            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, current_label, style);

            // Fill remaining space
            let text_len = current_label.chars().count() as u16;
            let fill_start = area.x + 1 + text_len;
            let chevron_pos = area.x + area.width.saturating_sub(2);

            for x in fill_start..chevron_pos {
                set_string(buf, x, area.y, " ", style);
            }

            set_string(buf, chevron_pos, area.y, TuiIcons::CHEVRON_DOWN, chevron_style);
            set_string(buf, chevron_pos + 1, area.y, "]", style);
        }
        DropdownVariant::Text { current_label, .. } => {
            let display = format!("[{}]", current_label);
            set_string(buf, area.x, area.y, &display, style);
        }
        DropdownVariant::IconTextChevron { current_icon, current_label, .. } => {
            let icon_char = icon_to_char(current_icon);
            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, icon_char, style);
            set_string(buf, area.x + 2, area.y, " ", style);
            set_string(buf, area.x + 3, area.y, current_label, style);

            let text_len = current_label.chars().count() as u16;
            let chevron_pos = area.x + area.width.saturating_sub(2);

            for x in (area.x + 3 + text_len)..chevron_pos {
                set_string(buf, x, area.y, " ", style);
            }

            set_string(buf, chevron_pos, area.y, TuiIcons::CHEVRON_DOWN, chevron_style);
            set_string(buf, chevron_pos + 1, area.y, "]", style);
        }
        DropdownVariant::IconChevron { current_icon, .. } => {
            let icon_char = icon_to_char(current_icon);
            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, icon_char, style);
            set_string(buf, area.x + 2, area.y, " ", style);
            set_string(buf, area.x + 3, area.y, TuiIcons::CHEVRON_DOWN, chevron_style);
            set_string(buf, area.x + 4, area.y, "]", style);
        }
        DropdownVariant::ChevronOnly { direction } => {
            let chevron = match direction {
                ChevronDirection::Up => TuiIcons::CHEVRON_UP,
                ChevronDirection::Down => TuiIcons::CHEVRON_DOWN,
                ChevronDirection::Left => TuiIcons::CHEVRON_LEFT,
                ChevronDirection::Right => TuiIcons::CHEVRON_RIGHT,
            };
            set_string(buf, area.x, area.y, "[", style);
            set_string(buf, area.x + 1, area.y, chevron, chevron_style);
            set_string(buf, area.x + 2, area.y, "]", style);
        }
    }
}

/// Helper: write a string to buffer at (col, row)
fn set_string(buf: &mut TerminalBuffer, col: u16, row: u16, text: &str, style: Style) {
    let mut x = col;
    for ch in text.chars() {
        if x >= buf.width() {
            break;
        }
        if row < buf.height() {
            buf.set(x, row, Cell::styled(ch.to_string(), style));
        }
        x += 1;
    }
}

/// Parse CSS hex color to terminal color
fn parse_color(hex: &str) -> Color {
    // Simple parser for #RRGGBB
    if hex.starts_with('#') && hex.len() == 7 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[1..3], 16),
            u8::from_str_radix(&hex[3..5], 16),
            u8::from_str_radix(&hex[5..7], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }

    // Fallback to indexed colors for common names
    match hex {
        "red" => Color::Red,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "yellow" => Color::Yellow,
        "cyan" => Color::Cyan,
        "magenta" => Color::Magenta,
        "white" => Color::White,
        "black" => Color::Black,
        _ => Color::Indexed(240), // Gray fallback
    }
}
