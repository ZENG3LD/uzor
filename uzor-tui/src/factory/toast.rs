//! Toast notification rendering to terminal cells

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Style, Modifier};
use crate::rect::Rect;
use uzor::widgets::toast::types::ToastType;
use super::defaults::*;

/// Render a toast notification to the terminal buffer
pub fn render_default(
    buf: &mut TerminalBuffer,
    toast: &ToastType,
    area: Rect,
) {
    let colors = TuiColors::default();

    let (icon, fg, message) = match toast {
        ToastType::Info { message, .. } => (TuiIcons::INFO, colors.fg_info, message.as_str()),
        ToastType::Success { message, .. } => (TuiIcons::CHECK, colors.fg_success, message.as_str()),
        ToastType::Warning { message, .. } => (TuiIcons::WARNING, colors.fg_warning, message.as_str()),
        ToastType::Error { message, .. } => (TuiIcons::CROSS, colors.fg_danger, message.as_str()),
    };

    let style = Style { fg, bg: colors.bg_active, modifiers: Modifier::BOLD };
    let text = format!(" {} {} ", icon, message);

    // Fill background
    for x in area.x..area.x + area.width {
        if x < buf.width() && area.y < buf.height() {
            buf.set(x, area.y, Cell::styled(" ", Style { fg: Color::Reset, bg: colors.bg_active, modifiers: Modifier::empty() }));
        }
    }

    // Write text
    set_string(buf, area.x, area.y, &text, style);
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
