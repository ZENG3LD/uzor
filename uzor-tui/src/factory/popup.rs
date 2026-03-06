//! Popup rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor_core::widgets::popup::types::PopupType;
use super::defaults::*;

/// Render a popup to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    popup: &PopupType,
    area: Rect,
) {
    let colors = TuiColors::default();

    match popup {
        PopupType::ContextMenu { selected_index, .. } => {
            // Draw border
            draw_border(buf, area, colors.border_focused);

            // Context menu items would be rendered by app code
            // This is just the container
            _ = selected_index;
        }
        PopupType::ColorPicker { selected_color, custom_mode, .. } => {
            // Draw border
            draw_border(buf, area, colors.border_focused);

            // Color palette grid - simple example
            render_color_palette(buf, area.inner(), &colors);

            _ = (selected_color, custom_mode);
        }
        PopupType::Custom { .. } => {
            // Generic popup with border
            draw_border(buf, area, colors.border_focused);
        }
    }
}

fn draw_border(buf: &mut TerminalBuffer, area: Rect, color: Color) {
    let style = Style { fg: color, bg: Color::Indexed(235), modifiers: Modifier::empty() };

    // Fill background
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if x < buf.width() && y < buf.height() {
                buf.set(x, y, Cell::styled(" ", style));
            }
        }
    }

    // Draw border box
    let border_style = Style { fg: color, bg: Color::Indexed(235), modifiers: Modifier::empty() };

    // Top/bottom
    for x in area.x..area.x + area.width {
        if x < buf.width() && area.y < buf.height() {
            buf.set(x, area.y, Cell::styled("─", border_style));
        }
        let bottom = area.y + area.height.saturating_sub(1);
        if x < buf.width() && bottom < buf.height() {
            buf.set(x, bottom, Cell::styled("─", border_style));
        }
    }

    // Left/right
    for y in area.y..area.y + area.height {
        if area.x < buf.width() && y < buf.height() {
            buf.set(area.x, y, Cell::styled("│", border_style));
        }
        let right = area.x + area.width.saturating_sub(1);
        if right < buf.width() && y < buf.height() {
            buf.set(right, y, Cell::styled("│", border_style));
        }
    }

    // Corners
    let right = area.x + area.width.saturating_sub(1);
    let bottom = area.y + area.height.saturating_sub(1);
    if area.x < buf.width() && area.y < buf.height() {
        buf.set(area.x, area.y, Cell::styled("┌", border_style));
    }
    if right < buf.width() && area.y < buf.height() {
        buf.set(right, area.y, Cell::styled("┐", border_style));
    }
    if area.x < buf.width() && bottom < buf.height() {
        buf.set(area.x, bottom, Cell::styled("└", border_style));
    }
    if right < buf.width() && bottom < buf.height() {
        buf.set(right, bottom, Cell::styled("┘", border_style));
    }
}

fn render_color_palette(buf: &mut TerminalBuffer, area: Rect, _colors: &TuiColors) {
    // Simple 8-color palette grid
    let palette = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::White,
    ];

    let mut x = area.x;
    let mut y = area.y;

    for color in &palette {
        if x >= area.x + area.width {
            x = area.x;
            y += 1;
        }
        if y >= area.y + area.height {
            break;
        }

        let style = Style { fg: Color::White, bg: *color, modifiers: Modifier::empty() };
        if x < buf.width() && y < buf.height() {
            buf.set(x, y, Cell::styled("█", style));
        }
        if x + 1 < buf.width() && y < buf.height() {
            buf.set(x + 1, y, Cell::styled("█", style));
        }
        x += 3; // spacing
    }
}
