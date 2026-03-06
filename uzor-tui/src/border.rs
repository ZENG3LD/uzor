//! Box-drawing border rendering.

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::rect::Rect;
use crate::style::Style;

/// Border line style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderType {
    #[default]
    Plain,
    Rounded,
    Double,
    Thick,
}

/// Unicode box-drawing character set.
#[derive(Debug, Clone, Copy)]
pub struct BorderChars {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
}

impl BorderType {
    pub const fn chars(self) -> BorderChars {
        match self {
            BorderType::Plain => BorderChars {
                top_left: '┌', top_right: '┐',
                bottom_left: '└', bottom_right: '┘',
                horizontal: '─', vertical: '│',
            },
            BorderType::Rounded => BorderChars {
                top_left: '╭', top_right: '╮',
                bottom_left: '╰', bottom_right: '╯',
                horizontal: '─', vertical: '│',
            },
            BorderType::Double => BorderChars {
                top_left: '╔', top_right: '╗',
                bottom_left: '╚', bottom_right: '╝',
                horizontal: '═', vertical: '║',
            },
            BorderType::Thick => BorderChars {
                top_left: '┏', top_right: '┓',
                bottom_left: '┗', bottom_right: '┛',
                horizontal: '━', vertical: '┃',
            },
        }
    }
}

/// Which sides of the border to draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Borders {
    pub top: bool,
    pub bottom: bool,
    pub left: bool,
    pub right: bool,
}

impl Borders {
    pub const ALL: Borders = Borders { top: true, bottom: true, left: true, right: true };
    pub const NONE: Borders = Borders { top: false, bottom: false, left: false, right: false };
    pub const TOP_BOTTOM: Borders = Borders { top: true, bottom: true, left: false, right: false };
    pub const LEFT_RIGHT: Borders = Borders { top: false, bottom: false, left: true, right: true };
}

impl Default for Borders {
    fn default() -> Self { Borders::NONE }
}

/// Render a border into the buffer at the given rect.
pub fn render_border(
    buf: &mut TerminalBuffer,
    area: Rect,
    border_type: BorderType,
    borders: Borders,
    style: Style,
) {
    if area.is_empty() {
        return;
    }

    let chars = border_type.chars();
    let x1 = area.left();
    let y1 = area.top();
    let x2 = area.right().saturating_sub(1); // last column
    let y2 = area.bottom().saturating_sub(1); // last row

    // Check bounds against buffer
    let buf_w = buf.width();
    let buf_h = buf.height();

    // Top border
    if borders.top && y1 < buf_h {
        for x in x1..area.right() {
            if x < buf_w {
                let ch = if x == x1 && borders.left {
                    chars.top_left
                } else if x == x2 && borders.right {
                    chars.top_right
                } else {
                    chars.horizontal
                };
                buf.set(x, y1, Cell::styled(&ch.to_string(), style));
            }
        }
    }

    // Bottom border
    if borders.bottom && y2 < buf_h && area.height > 1 {
        for x in x1..area.right() {
            if x < buf_w {
                let ch = if x == x1 && borders.left {
                    chars.bottom_left
                } else if x == x2 && borders.right {
                    chars.bottom_right
                } else {
                    chars.horizontal
                };
                buf.set(x, y2, Cell::styled(&ch.to_string(), style));
            }
        }
    }

    // Left border
    if borders.left && x1 < buf_w {
        let start_y = if borders.top { y1 + 1 } else { y1 };
        let end_y = if borders.bottom && area.height > 1 { y2 } else { area.bottom() };
        for y in start_y..end_y {
            if y < buf_h {
                buf.set(x1, y, Cell::styled(&chars.vertical.to_string(), style));
            }
        }
    }

    // Right border
    if borders.right && x2 < buf_w && area.width > 1 {
        let start_y = if borders.top { y1 + 1 } else { y1 };
        let end_y = if borders.bottom && area.height > 1 { y2 } else { area.bottom() };
        for y in start_y..end_y {
            if y < buf_h {
                buf.set(x2, y, Cell::styled(&chars.vertical.to_string(), style));
            }
        }
    }
}

/// Render a border with a title in the top edge.
///
/// Title is placed after the top-left corner with 1 cell padding on each side.
/// Truncated if too long.
pub fn render_border_with_title(
    buf: &mut TerminalBuffer,
    area: Rect,
    border_type: BorderType,
    borders: Borders,
    border_style: Style,
    title: &str,
    title_style: Style,
) {
    // First render the border normally
    render_border(buf, area, border_type, borders, border_style);

    // Then overwrite the top edge with the title
    if !borders.top || area.width < 4 || area.is_empty() {
        return;
    }

    let buf_w = buf.width();
    let buf_h = buf.height();
    let y = area.top();
    if y >= buf_h {
        return;
    }

    // Title starts at x+2 (after corner + space), max width is area.width - 4
    let title_start = area.left().saturating_add(2);
    let max_title_width = (area.width as usize).saturating_sub(4);

    let mut col = title_start;
    for ch in title.chars() {
        if (col as usize - title_start as usize) >= max_title_width || col >= buf_w {
            break;
        }
        buf.set(col, y, Cell::styled(&ch.to_string(), title_style));
        col += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::TerminalBuffer;

    #[test]
    fn test_border_chars_plain() {
        let chars = BorderType::Plain.chars();
        assert_eq!(chars.top_left, '┌');
        assert_eq!(chars.horizontal, '─');
        assert_eq!(chars.vertical, '│');
    }

    #[test]
    fn test_border_chars_rounded() {
        let chars = BorderType::Rounded.chars();
        assert_eq!(chars.top_left, '╭');
        assert_eq!(chars.bottom_right, '╯');
    }

    #[test]
    fn test_render_border_all() {
        let mut buf = TerminalBuffer::new(10, 5);
        let area = Rect::new(0, 0, 10, 5);
        render_border(&mut buf, area, BorderType::Plain, Borders::ALL, Style::default());

        assert_eq!(buf.get(0, 0).symbol, "┌");
        assert_eq!(buf.get(9, 0).symbol, "┐");
        assert_eq!(buf.get(0, 4).symbol, "└");
        assert_eq!(buf.get(9, 4).symbol, "┘");
        assert_eq!(buf.get(5, 0).symbol, "─");
        assert_eq!(buf.get(0, 2).symbol, "│");
        assert_eq!(buf.get(9, 2).symbol, "│");
    }

    #[test]
    fn test_render_border_with_title() {
        let mut buf = TerminalBuffer::new(20, 5);
        let area = Rect::new(0, 0, 20, 5);
        render_border_with_title(
            &mut buf, area,
            BorderType::Rounded, Borders::ALL,
            Style::default(), "Chat", Style::default(),
        );

        assert_eq!(buf.get(0, 0).symbol, "╭");
        assert_eq!(buf.get(2, 0).symbol, "C");
        assert_eq!(buf.get(3, 0).symbol, "h");
        assert_eq!(buf.get(4, 0).symbol, "a");
        assert_eq!(buf.get(5, 0).symbol, "t");
    }

    #[test]
    fn test_render_border_empty() {
        let mut buf = TerminalBuffer::new(10, 5);
        // Should not panic
        render_border(&mut buf, Rect::ZERO, BorderType::Plain, Borders::ALL, Style::default());
    }
}
