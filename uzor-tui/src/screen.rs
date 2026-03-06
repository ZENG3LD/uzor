//! Double-buffered screen with automatic diffing and resize debounce.

use std::io;
use crate::backend::Backend;
use crate::buffer::TerminalBuffer;

/// Double-buffered screen that only writes changed cells.
///
/// Tracks terminal dimensions so that rapid resize events (e.g. the user
/// dragging a window edge) are debounced: buffers are only rebuilt when the
/// size actually changes, and the first frame after a resize is drawn in
/// full (skipping the diff) to guarantee a clean picture.
pub struct Screen<B: Backend> {
    backend: B,
    /// What is currently displayed on the terminal.
    current: TerminalBuffer,
    /// What we want to display next.
    next: TerminalBuffer,
    /// Last known terminal size (width, height).
    last_size: (u16, u16),
    /// When true, the next `flush()` draws every cell instead of diffing.
    force_full_redraw: bool,
}

impl<B: Backend> Screen<B> {
    /// Create a new screen with the given backend and dimensions.
    pub fn new(backend: B, width: u16, height: u16) -> Self {
        Self {
            backend,
            current: TerminalBuffer::new(width, height),
            next: TerminalBuffer::new(width, height),
            last_size: (width, height),
            force_full_redraw: false,
        }
    }

    /// Get a mutable reference to the next buffer for writing.
    pub fn buffer_mut(&mut self) -> &mut TerminalBuffer {
        &mut self.next
    }

    /// Get a reference to the next buffer.
    pub fn buffer(&self) -> &TerminalBuffer {
        &self.next
    }

    /// Get the backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Get mutable backend.
    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    /// Current screen dimensions (width, height).
    pub fn size(&self) -> (u16, u16) {
        self.last_size
    }

    /// Notify the screen that the terminal has been resized.
    ///
    /// If the new size matches `last_size` the call is a no-op (debounce).
    /// Otherwise both buffers are rebuilt at the new dimensions and the
    /// next `flush()` will perform a full redraw.
    pub fn resize(&mut self, width: u16, height: u16) {
        if (width, height) == self.last_size {
            return;
        }

        self.current.resize(width, height);
        self.next.resize(width, height);
        self.last_size = (width, height);
        self.force_full_redraw = true;
    }

    /// Diff next vs current, draw only changes, then swap buffers.
    ///
    /// When `force_full_redraw` is set (after a resize) the terminal is
    /// cleared first and every cell from the next buffer is drawn,
    /// bypassing the diff entirely. The flag is then reset.
    pub fn flush(&mut self) -> io::Result<()> {
        if self.force_full_redraw {
            self.force_full_redraw = false;

            let cells = self.next.all_cells();
            self.backend.begin_sync()?;
            self.backend.clear()?;
            self.backend.draw(cells.into_iter())?;
            self.backend.flush()?;
            self.backend.end_sync()?;
            self.backend.flush()?;
        } else {
            let changes = self.next.diff(&self.current);

            if !changes.is_empty() {
                self.backend.begin_sync()?;
                self.backend.draw(changes.into_iter())?;
                self.backend.flush()?;
                self.backend.end_sync()?;
                self.backend.flush()?;
            }
        }

        // Swap: current = next
        std::mem::swap(&mut self.current, &mut self.next);

        // Clear dirty flags on both buffers so the next frame only diffs
        // rows that are actually written to.
        self.current.clear_dirty();
        self.next.clear_dirty();

        Ok(())
    }
}
