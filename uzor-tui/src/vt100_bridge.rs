//! Bridge from vt100 virtual terminal to TerminalBuffer.

use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::style::{Color, Modifier, Style};

/// Copy the entire vt100 screen into a TerminalBuffer.
///
/// Reads every cell from the vt100 screen and writes it into the buffer,
/// converting colors and attributes to our types.
pub fn vt100_to_buffer(screen: &vt100::Screen, buf: &mut TerminalBuffer) {
    let (rows, cols) = screen.size();

    for row in 0..rows {
        for col in 0..cols {
            let vt_cell = screen.cell(row, col);
            let cell = match vt_cell {
                Some(vc) => convert_cell(vc),
                None => Cell::default(),
            };
            // buffer uses (col, row) ordering
            if col < buf.width() && row < buf.height() {
                buf.set(col, row, cell);
            }
        }
    }
}

/// Convert a vt100 cell to our Cell.
fn convert_cell(vc: &vt100::Cell) -> Cell {
    let symbol = vc.contents();
    // Empty string means no content (use space)
    let symbol: compact_str::CompactString = if symbol.is_empty() {
        " ".into()
    } else {
        symbol.into()
    };

    let fg = convert_color(vc.fgcolor());
    let bg = convert_color(vc.bgcolor());

    let mut modifiers = Modifier::empty();
    if vc.bold() {
        modifiers |= Modifier::BOLD;
    }
    if vc.italic() {
        modifiers |= Modifier::ITALIC;
    }
    if vc.underline() {
        modifiers |= Modifier::UNDERLINE;
    }
    if vc.inverse() {
        modifiers |= Modifier::REVERSE;
    }

    let style = Style {
        fg,
        bg,
        modifiers,
    };

    Cell { symbol, style }
}

/// Convert vt100 color to our Color.
fn convert_color(color: vt100::Color) -> Color {
    match color {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => {
            // Map standard 16 colors to named colors
            match i {
                0 => Color::Black,
                1 => Color::Red,
                2 => Color::Green,
                3 => Color::Yellow,
                4 => Color::Blue,
                5 => Color::Magenta,
                6 => Color::Cyan,
                7 => Color::White,
                // 8-15: bright variants, use indexed
                _ => Color::Indexed(i),
            }
        }
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_screen_to_buffer() {
        let parser = vt100::Parser::new(24, 80, 0);
        let mut buf = TerminalBuffer::new(80, 24);
        vt100_to_buffer(parser.screen(), &mut buf);

        // All cells should be spaces
        assert_eq!(buf.get(0, 0).symbol, " ");
        assert_eq!(buf.get(79, 23).symbol, " ");
    }

    #[test]
    fn test_text_to_buffer() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Hello, world!");
        let mut buf = TerminalBuffer::new(80, 24);
        vt100_to_buffer(parser.screen(), &mut buf);

        assert_eq!(buf.get(0, 0).symbol, "H");
        assert_eq!(buf.get(1, 0).symbol, "e");
        assert_eq!(buf.get(4, 0).symbol, "o");
        assert_eq!(buf.get(12, 0).symbol, "!");
    }

    #[test]
    fn test_colored_text_to_buffer() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[31m = red foreground, then "Hi", then reset ESC[0m
        parser.process(b"\x1b[31mHi\x1b[0m");
        let mut buf = TerminalBuffer::new(80, 24);
        vt100_to_buffer(parser.screen(), &mut buf);

        assert_eq!(buf.get(0, 0).symbol, "H");
        assert_eq!(buf.get(0, 0).style.fg, Color::Red);
        assert_eq!(buf.get(1, 0).symbol, "i");
        assert_eq!(buf.get(1, 0).style.fg, Color::Red);
        // After reset, next cell should be default
        assert_eq!(buf.get(2, 0).style.fg, Color::Reset);
    }

    #[test]
    fn test_bold_text_to_buffer() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[1m = bold
        parser.process(b"\x1b[1mBold\x1b[0m");
        let mut buf = TerminalBuffer::new(80, 24);
        vt100_to_buffer(parser.screen(), &mut buf);

        assert!(buf.get(0, 0).style.modifiers.contains(Modifier::BOLD));
        assert_eq!(buf.get(0, 0).symbol, "B");
    }
}
