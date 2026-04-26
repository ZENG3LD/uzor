//! Backend trait for terminal rendering.

mod crossterm;

pub use self::crossterm::CrosstermBackend;

use std::io;
use crate::cell::Cell;

/// Trait for rendering terminal content.
pub trait Backend {
    /// Draw changed cells to the terminal.
    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>;

    /// Hide the cursor.
    fn hide_cursor(&mut self) -> io::Result<()>;

    /// Show the cursor.
    fn show_cursor(&mut self) -> io::Result<()>;

    /// Clear the entire screen.
    fn clear(&mut self) -> io::Result<()>;

    /// Get terminal size (cols, rows).
    fn size(&self) -> io::Result<(u16, u16)>;

    /// Flush output to the terminal.
    fn flush(&mut self) -> io::Result<()>;

    /// Begin synchronized update (DEC mode 2026).
    ///
    /// Tells the terminal to buffer subsequent output and render it
    /// atomically, preventing tearing during frame draws.
    fn begin_sync(&mut self) -> io::Result<()> {
        Ok(())
    }

    /// End synchronized update (DEC mode 2026).
    ///
    /// Signals the terminal to flush its buffered output and render
    /// the complete frame at once.
    fn end_sync(&mut self) -> io::Result<()> {
        Ok(())
    }
}
