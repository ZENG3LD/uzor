//! Frame + Terminal loop, plus an in-memory backend for dumps/tests.

use std::io;

use crate::backend::{Backend, CrosstermBackend};
use crate::buffer::TerminalBuffer;
use crate::cell::Cell;
use crate::rect::Rect;
use crate::screen::Screen;
use crate::widget::Widget;

/// One draw pass: widgets write into the next buffer.
pub struct Frame<'a> {
    buf: &'a mut TerminalBuffer,
    area: Rect,
}

impl<'a> Frame<'a> {
    pub fn area(&self) -> Rect {
        self.area
    }

    pub fn buffer_mut(&mut self) -> &mut TerminalBuffer {
        self.buf
    }

    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        widget.render(area, self.buf);
    }
}

/// Double-buffered terminal. `draw` clears the next buffer, runs the closure, flushes.
pub struct Terminal<B: Backend> {
    screen: Screen<B>,
}

impl<B: Backend> Terminal<B> {
    pub fn new(backend: B) -> io::Result<Self> {
        let (w, h) = backend.size()?;
        let mut term = Self {
            screen: Screen::new(backend, w.max(1), h.max(1)),
        };
        // Ratatui hid the caret on enter. This shell is display-first;
        // a blinking block in a table is not an input caret.
        term.screen.backend_mut().hide_cursor()?;
        Ok(term)
    }

    pub fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame<'_>),
    {
        let (w, h) = self.screen.backend().size()?;
        self.screen.resize(w.max(1), h.max(1));
        let _ = self.screen.backend_mut().hide_cursor();
        self.screen.buffer_mut().clear();
        let area = Rect::new(0, 0, w.max(1), h.max(1));
        {
            let buf = self.screen.buffer_mut();
            let mut frame = Frame { buf, area };
            f(&mut frame);
        }
        self.screen.flush()
    }

    pub fn backend(&self) -> &B {
        self.screen.backend()
    }

    pub fn backend_mut(&mut self) -> &mut B {
        self.screen.backend_mut()
    }

    /// Last flushed cell grid.
    pub fn current_buffer(&self) -> &TerminalBuffer {
        self.screen.current()
    }
}

impl<B: Backend> Drop for Terminal<B> {
    fn drop(&mut self) {
        let _ = self.screen.backend_mut().show_cursor();
    }
}

impl Terminal<CrosstermBackend<std::io::Stdout>> {
    pub fn stdout() -> io::Result<Self> {
        Terminal::new(CrosstermBackend::new(std::io::stdout()))
    }
}

/// In-memory backend. `draw` is a no-op: the `Screen` already holds the cells.
pub struct TestBackend {
    width: u16,
    height: u16,
}

impl TestBackend {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
        }
    }
}

impl Backend for TestBackend {
    fn draw<'a, I>(&mut self, _content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        Ok(())
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn size(&self) -> io::Result<(u16, u16)> {
        Ok((self.width, self.height))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Color, Style};
    use crate::text::Line;
    use crate::widget::Paragraph;

    #[test]
    fn draw_then_read_current() {
        let mut term = Terminal::new(TestBackend::new(12, 3)).unwrap();
        term.draw(|f| {
            f.render_widget(Paragraph::new(Line::from("hi")), f.area());
        })
        .unwrap();
        let buf = term.current_buffer();
        assert_eq!(buf.get(0, 0).symbol, "h");
        assert_eq!(buf.get(1, 0).symbol, "i");
        assert_eq!(buf.get(0, 0).style.fg, Color::Reset);
        let _ = Style::default();
    }
}
