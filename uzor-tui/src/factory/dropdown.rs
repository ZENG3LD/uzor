//! Dropdown rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor_core::widgets::dropdown::types::DropdownType;
use uzor_core::types::WidgetState;
use super::defaults::*;

/// Render a dropdown to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    dropdown: &DropdownType,
    area: Rect,
    _state: WidgetState,
) {
    let colors = TuiColors::default();

    match dropdown {
        DropdownType::Standard { selected_index, placeholder, .. } => {
            // Closed state: [Placeholder ▼]
            let display = if selected_index.is_some() {
                "Item Selected"
            } else {
                placeholder.as_str()
            };

            render_closed_dropdown(buf, area, display, &colors);
        }
        DropdownType::Grid { selected_index, columns, .. } => {
            // Grid dropdown: [Grid ▼]
            let display = if selected_index.is_some() {
                "Grid Item"
            } else {
                "Select Grid"
            };

            render_closed_dropdown(buf, area, display, &colors);

            // Show columns indicator
            let info_style = Style { fg: colors.fg_dimmed, bg: Color::Reset, modifiers: Modifier::empty() };
            let cols_text = format!(" {}col", columns);
            set_string(buf, area.x + area.width.saturating_sub(8), area.y, &cols_text, info_style);
        }
        DropdownType::Layout { selected_index, .. } => {
            // Layout dropdown: [Layout ▼]
            let display = if selected_index.is_some() {
                "Layout Selected"
            } else {
                "Select Layout"
            };

            render_closed_dropdown(buf, area, display, &colors);
        }
    }
}

fn render_closed_dropdown(buf: &mut TerminalBuffer, area: Rect, text: &str, colors: &TuiColors) {
    let style = Style { fg: colors.fg_normal, bg: Color::Reset, modifiers: Modifier::empty() };
    let chevron_style = Style { fg: colors.fg_dimmed, bg: Color::Reset, modifiers: Modifier::empty() };

    // [Text............▼]
    set_string(buf, area.x, area.y, "[", style);
    set_string(buf, area.x + 1, area.y, text, style);

    let text_len = text.chars().count() as u16;
    let fill_start = area.x + 1 + text_len;
    let chevron_pos = area.x + area.width.saturating_sub(2);

    // Fill space
    for x in fill_start..chevron_pos {
        if x < buf.width() && area.y < buf.height() {
            buf.set(x, area.y, Cell::styled(".", Style { fg: colors.fg_dimmed, bg: Color::Reset, modifiers: Modifier::empty() }));
        }
    }

    set_string(buf, chevron_pos, area.y, TuiIcons::CHEVRON_DOWN, chevron_style);
    set_string(buf, chevron_pos + 1, area.y, "]", style);
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
