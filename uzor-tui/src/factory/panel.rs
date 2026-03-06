//! Panel rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor_core::widgets::panel::types::*;
use uzor_core::types::WidgetState;
use super::defaults::*;

/// Render a panel to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    panel: &PanelType,
    area: Rect,
    _state: WidgetState,
) {
    let colors = TuiColors::default();

    match panel {
        PanelType::Toolbar { variant, .. } => {
            render_toolbar(buf, *variant, area, &colors);
        }
        PanelType::Sidebar { variant, .. } => {
            render_sidebar(buf, *variant, area, &colors);
        }
        PanelType::Modal { variant, .. } => {
            render_modal(buf, *variant, area, &colors);
        }
        PanelType::Hideable { is_hidden, .. } => {
            if !is_hidden {
                draw_border(buf, area, colors.border_normal);
            }
        }
    }
}

fn render_toolbar(buf: &mut TerminalBuffer, variant: ToolbarVariant, area: Rect, colors: &TuiColors) {
    // Fill with background color
    let bg_style = Style { fg: colors.fg_normal, bg: colors.bg_active, modifiers: Modifier::empty() };

    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if x < buf.width() && y < buf.height() {
                buf.set(x, y, Cell::styled(" ", bg_style));
            }
        }
    }

    // Add position indicator
    let label = match variant {
        ToolbarVariant::Top => " Top Toolbar ",
        ToolbarVariant::Bottom => " Bottom Toolbar ",
        ToolbarVariant::Left => " Left ",
        ToolbarVariant::Right => " Right ",
    };

    set_string(buf, area.x + 1, area.y, label, Style { fg: colors.fg_dimmed, bg: colors.bg_active, modifiers: Modifier::empty() });
}

fn render_sidebar(buf: &mut TerminalBuffer, variant: SidebarVariant, area: Rect, colors: &TuiColors) {
    // Draw border
    draw_border(buf, area, colors.border_normal);

    // Title
    let title = match variant {
        SidebarVariant::Left => "Left Sidebar",
        SidebarVariant::Right => "Right Sidebar",
        SidebarVariant::Bottom => "Bottom Sidebar",
    };

    let title_style = Style { fg: colors.fg_normal, bg: Color::Reset, modifiers: Modifier::BOLD };
    set_string(buf, area.x + 2, area.y, title, title_style);
}

fn render_modal(buf: &mut TerminalBuffer, variant: ModalVariant, area: Rect, colors: &TuiColors) {
    // Fill background
    let bg_style = Style { fg: Color::Reset, bg: Color::Indexed(235), modifiers: Modifier::empty() };
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if x < buf.width() && y < buf.height() {
                buf.set(x, y, Cell::styled(" ", bg_style));
            }
        }
    }

    // Draw border
    draw_border(buf, area, colors.border_focused);

    // Title
    let title = match variant {
        ModalVariant::Search => " Search ",
        ModalVariant::Settings => " Settings ",
        ModalVariant::Simple => " Modal ",
        ModalVariant::Primitive => " Primitive Properties ",
    };

    let title_style = Style { fg: colors.fg_accent, bg: Color::Indexed(235), modifiers: Modifier::BOLD };
    set_string(buf, area.x + 2, area.y, title, title_style);
}

fn draw_border(buf: &mut TerminalBuffer, area: Rect, color: Color) {
    let style = Style { fg: color, bg: Color::Reset, modifiers: Modifier::empty() };

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
