//! Container rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor::widgets::container::types::ContainerType;
use uzor::types::WidgetState;
use super::defaults::*;

/// Render a container to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    container: &ContainerType,
    area: Rect,
    _state: WidgetState,
) {
    let colors = TuiColors::default();

    match container {
        ContainerType::Scrollable { scroll_offset, content_height, viewport_height, .. } => {
            // Draw border (simple box)
            draw_border(buf, area, colors.border_normal);

            // Draw scrollbar if needed
            if content_height > viewport_height {
                render_scrollbar(buf, area, *scroll_offset, *content_height, *viewport_height, &colors);
            }
        }
        ContainerType::Plain { .. } => {
            // Simple border
            draw_border(buf, area, colors.border_normal);
        }
    }
}

fn draw_border(buf: &mut TerminalBuffer, area: Rect, color: Color) {
    let style = Style { fg: color, bg: Color::Reset, modifiers: Modifier::empty() };

    // Top border
    for x in area.x..area.x + area.width {
        if x < buf.width() && area.y < buf.height() {
            buf.set(x, area.y, Cell::styled("─", style));
        }
    }

    // Bottom border
    let bottom = area.y + area.height.saturating_sub(1);
    for x in area.x..area.x + area.width {
        if x < buf.width() && bottom < buf.height() {
            buf.set(x, bottom, Cell::styled("─", style));
        }
    }

    // Left border
    for y in area.y..area.y + area.height {
        if area.x < buf.width() && y < buf.height() {
            buf.set(area.x, y, Cell::styled("│", style));
        }
    }

    // Right border
    let right = area.x + area.width.saturating_sub(1);
    for y in area.y..area.y + area.height {
        if right < buf.width() && y < buf.height() {
            buf.set(right, y, Cell::styled("│", style));
        }
    }

    // Corners
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

fn render_scrollbar(
    buf: &mut TerminalBuffer,
    area: Rect,
    scroll_offset: f64,
    content_height: f64,
    viewport_height: f64,
    colors: &TuiColors,
) {
    let scrollbar_x = area.x + area.width.saturating_sub(1);
    let track_start_y = area.y + 1;
    let track_height = area.height.saturating_sub(2);

    if track_height == 0 {
        return;
    }

    let visible_ratio = viewport_height / content_height;
    let thumb_height = (visible_ratio * track_height as f64).max(1.0) as u16;
    let max_scroll = (content_height - viewport_height).max(0.0);

    let thumb_offset = if max_scroll > 0.0 {
        ((scroll_offset / max_scroll) * (track_height - thumb_height) as f64) as u16
    } else {
        0
    };

    let track_style = Style { fg: colors.fg_dimmed, bg: Color::Reset, modifiers: Modifier::empty() };
    let thumb_style = Style { fg: colors.fg_accent, bg: Color::Reset, modifiers: Modifier::empty() };

    // Draw track
    for i in 0..track_height {
        let y = track_start_y + i;
        if scrollbar_x < buf.width() && y < buf.height() {
            buf.set(scrollbar_x, y, Cell::styled(TuiIcons::SCROLLBAR_TRACK, track_style));
        }
    }

    // Draw thumb
    for i in 0..thumb_height {
        let y = track_start_y + thumb_offset + i;
        if scrollbar_x < buf.width() && y < buf.height() {
            buf.set(scrollbar_x, y, Cell::styled(TuiIcons::SCROLLBAR_THUMB, thumb_style));
        }
    }
}
