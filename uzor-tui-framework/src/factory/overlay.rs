//! Overlay rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor::widgets::overlay::types::OverlayType;
use super::defaults::*;

/// Render an overlay to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    overlay: &OverlayType,
    area: Rect,
) {
    let colors = TuiColors::default();

    match overlay {
        OverlayType::Tooltip { text, .. } => {
            // Simple tooltip: bordered text
            let bg_style = Style { fg: colors.fg_normal, bg: Color::Indexed(235), modifiers: Modifier::empty() };

            // Fill background
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    if x < buf.width() && y < buf.height() {
                        buf.set(x, y, Cell::styled(" ", bg_style));
                    }
                }
            }

            // Draw border
            draw_thin_border(buf, area, colors.fg_dimmed);

            // Text
            let text_style = Style { fg: colors.fg_normal, bg: Color::Indexed(235), modifiers: Modifier::empty() };
            set_string(buf, area.x + 1, area.y + 1, text, text_style);
        }
        OverlayType::InfoOverlay { text, .. } => {
            // Info overlay: larger box with content
            let bg_style = Style { fg: Color::Reset, bg: Color::Indexed(235), modifiers: Modifier::empty() };

            // Fill background
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    if x < buf.width() && y < buf.height() {
                        buf.set(x, y, Cell::styled(" ", bg_style));
                    }
                }
            }

            // Draw border
            draw_thin_border(buf, area, colors.border_focused);

            // Content
            let content_style = Style { fg: colors.fg_info, bg: Color::Indexed(235), modifiers: Modifier::empty() };
            set_string(buf, area.x + 1, area.y + 1, text, content_style);
        }
    }
}

fn draw_thin_border(buf: &mut TerminalBuffer, area: Rect, color: Color) {
    let style = Style { fg: color, bg: Color::Indexed(235), modifiers: Modifier::empty() };

    // Top/bottom
    for x in area.x..area.x + area.width {
        if x < buf.width() && area.y < buf.height() {
            buf.set(x, area.y, Cell::styled("─", style));
        }
        let bottom = area.y + area.height.saturating_sub(1);
        if x < buf.width() && bottom < buf.height() {
            buf.set(x, bottom, Cell::styled("─", style));
        }
    }

    // Left/right
    for y in area.y..area.y + area.height {
        if area.x < buf.width() && y < buf.height() {
            buf.set(area.x, y, Cell::styled("│", style));
        }
        let right = area.x + area.width.saturating_sub(1);
        if right < buf.width() && y < buf.height() {
            buf.set(right, y, Cell::styled("│", style));
        }
    }

    // Corners
    let right = area.x + area.width.saturating_sub(1);
    let bottom = area.y + area.height.saturating_sub(1);
    if area.x < buf.width() && area.y < buf.height() {
        buf.set(area.x, area.y, Cell::styled("┌", style));
    }
    if right < buf.width() && area.y < buf.height() {
        buf.set(right, area.y, Cell::styled("┐", style));
    }
    if area.x < buf.width() && bottom < buf.height() {
        buf.set(area.x, bottom, Cell::styled("└", style));
    }
    if right < buf.width() && bottom < buf.height() {
        buf.set(right, bottom, Cell::styled("┘", style));
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
