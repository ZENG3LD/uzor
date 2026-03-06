//! Text input rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor_core::widgets::text_input::types::TextInputType;
use uzor_core::types::WidgetState;
use super::defaults::*;

/// Render a text input to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    input: &TextInputType,
    area: Rect,
    _state: WidgetState,
    cursor_pos: usize,
) {
    let colors = TuiColors::default();

    let (value, placeholder, focused, disabled) = match input {
        TextInputType::Text { value, placeholder, focused, disabled, .. } => {
            (value.as_str(), placeholder.as_str(), *focused, *disabled)
        }
        TextInputType::Number { value, placeholder, focused, disabled, .. } => {
            (value.as_str(), placeholder.as_str(), *focused, *disabled)
        }
        TextInputType::Search { value, placeholder, focused, .. } => {
            (value.as_str(), placeholder.as_str(), *focused, false)
        }
        TextInputType::Password { value: _, placeholder, focused, .. } => {
            // Show dots instead of actual text
            ("", placeholder.as_str(), *focused, false)
        }
    };

    let border_color = if focused {
        colors.border_focused
    } else if disabled {
        colors.fg_disabled
    } else {
        colors.border_normal
    };

    let fg = if disabled {
        colors.fg_disabled
    } else {
        colors.fg_normal
    };

    let bg = Color::Reset;
    let text_style = Style { fg, bg, modifiers: Modifier::empty() };
    let border_style = Style { fg: border_color, bg, modifiers: Modifier::empty() };

    // Draw input: > [text___]
    set_string(buf, area.x, area.y, ">", border_style);
    set_string(buf, area.x + 1, area.y, " [", border_style);

    let input_start = area.x + 3;
    let input_width = area.width.saturating_sub(5); // "> [" and "]"

    // Show value or placeholder
    let display_text = if value.is_empty() {
        placeholder
    } else {
        value
    };

    // Render password as dots
    let display = if matches!(input, TextInputType::Password { .. }) && !value.is_empty() {
        "•".repeat(value.len())
    } else {
        display_text.to_string()
    };

    // Draw text
    for (i, ch) in display.chars().enumerate() {
        let x = input_start + i as u16;
        if x >= input_start + input_width {
            break;
        }

        let mut char_style = text_style;

        // Cursor highlighting
        if focused && i == cursor_pos {
            char_style.modifiers |= Modifier::REVERSE;
        }

        if x < buf.width() && area.y < buf.height() {
            buf.set(x, area.y, Cell::styled(ch.to_string(), char_style));
        }
    }

    // Fill remaining space with underscores or spaces
    let text_len = display.chars().count();
    for i in text_len..(input_width as usize) {
        let x = input_start + i as u16;
        if x >= input_start + input_width {
            break;
        }

        let mut char_style = text_style;

        // Cursor at end
        if focused && i == cursor_pos {
            char_style.modifiers |= Modifier::REVERSE;
        }

        let fill_char = if value.is_empty() { " " } else { "_" };

        if x < buf.width() && area.y < buf.height() {
            buf.set(x, area.y, Cell::styled(fill_char, char_style));
        }
    }

    // Closing bracket
    let bracket_x = input_start + input_width;
    if bracket_x < buf.width() && area.y < buf.height() {
        buf.set(bracket_x, area.y, Cell::styled("]", border_style));
    }

    // Icon for search input
    if matches!(input, TextInputType::Search { .. }) {
        let search_icon_style = Style { fg: colors.fg_dimmed, bg, modifiers: Modifier::empty() };
        set_string(buf, input_start + input_width + 1, area.y, " 🔍", search_icon_style);
    }
}

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
