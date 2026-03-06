//! Crossterm-based terminal backend.

use std::io::{self, Write};
use crossterm::{
    cursor,
    execute,
    queue,
    style::{Attribute, Color as CtColor, SetAttribute, SetBackgroundColor, SetForegroundColor, Print},
    terminal,
};

use crate::cell::Cell;
use crate::style::{Color, Modifier};
use super::Backend;

/// Backend that renders to a terminal via crossterm.
pub struct CrosstermBackend<W: Write> {
    writer: W,
}

impl<W: Write> CrosstermBackend<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> Backend for CrosstermBackend<W> {
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        let mut last_style: Option<crate::style::Style> = None;
        // Track cursor position for ANSI coalescing: skip MoveTo when the
        // cursor is already at the right position (consecutive cells on the
        // same row).
        let mut last_pos: Option<(u16, u16)> = None; // (col, row)

        for (col, row, cell) in content {
            // Only emit MoveTo when the cursor is NOT already at (col, row).
            // After printing a character the terminal advances the cursor by
            // one column, so adjacent cells in a horizontal run need no
            // repositioning.
            let cursor_already_here = last_pos
                .map_or(false, |(lc, lr)| lr == row && lc + 1 == col);
            if !cursor_already_here {
                queue!(self.writer, cursor::MoveTo(col, row))?;
            }

            // Only emit style escape sequences when the style actually changes.
            let need_style = last_style.map_or(true, |s| s != cell.style);
            if need_style {
                // Reset first
                queue!(self.writer, SetAttribute(Attribute::Reset))?;

                // Foreground
                if cell.style.fg != Color::Reset {
                    queue!(self.writer, SetForegroundColor(to_crossterm_color(cell.style.fg)))?;
                }

                // Background
                if cell.style.bg != Color::Reset {
                    queue!(self.writer, SetBackgroundColor(to_crossterm_color(cell.style.bg)))?;
                }

                // Modifiers
                let m = cell.style.modifiers;
                if m.contains(Modifier::BOLD) {
                    queue!(self.writer, SetAttribute(Attribute::Bold))?;
                }
                if m.contains(Modifier::DIM) {
                    queue!(self.writer, SetAttribute(Attribute::Dim))?;
                }
                if m.contains(Modifier::ITALIC) {
                    queue!(self.writer, SetAttribute(Attribute::Italic))?;
                }
                if m.contains(Modifier::UNDERLINE) {
                    queue!(self.writer, SetAttribute(Attribute::Underlined))?;
                }
                if m.contains(Modifier::BLINK) {
                    queue!(self.writer, SetAttribute(Attribute::SlowBlink))?;
                }
                if m.contains(Modifier::REVERSE) {
                    queue!(self.writer, SetAttribute(Attribute::Reverse))?;
                }
                if m.contains(Modifier::HIDDEN) {
                    queue!(self.writer, SetAttribute(Attribute::Hidden))?;
                }
                if m.contains(Modifier::STRIKETHROUGH) {
                    queue!(self.writer, SetAttribute(Attribute::CrossedOut))?;
                }

                last_style = Some(cell.style);
            }

            // Print the character
            queue!(self.writer, Print(&cell.symbol))?;

            // Update cursor tracking — terminal advances cursor by one after
            // printing, so the effective position is (col + 1, row). We store
            // (col, row) and check for lc + 1 == col on the next iteration,
            // which is equivalent.
            last_pos = Some((col, row));
        }

        // Reset style at the end
        queue!(self.writer, SetAttribute(Attribute::Reset))?;

        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, cursor::Hide)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(self.writer, cursor::Show)
    }

    fn clear(&mut self) -> io::Result<()> {
        execute!(self.writer, terminal::Clear(terminal::ClearType::All))
    }

    fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    fn begin_sync(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\x1b[?2026h")
    }

    fn end_sync(&mut self) -> io::Result<()> {
        self.writer.write_all(b"\x1b[?2026l")
    }
}

/// Convert our Color to crossterm Color.
fn to_crossterm_color(color: Color) -> CtColor {
    match color {
        Color::Reset => CtColor::Reset,
        Color::Black => CtColor::Black,
        Color::Red => CtColor::DarkRed,
        Color::Green => CtColor::DarkGreen,
        Color::Yellow => CtColor::DarkYellow,
        Color::Blue => CtColor::DarkBlue,
        Color::Magenta => CtColor::DarkMagenta,
        Color::Cyan => CtColor::DarkCyan,
        Color::White => CtColor::White,
        Color::Indexed(i) => CtColor::AnsiValue(i),
        Color::Rgb(r, g, b) => CtColor::Rgb { r, g, b },
    }
}
