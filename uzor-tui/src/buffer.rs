//! 2D terminal buffer with cell-level diff.

use crate::cell::Cell;

/// A 2D grid of terminal cells (row-major).
///
/// Tracks dirty rows internally so that `diff()` can skip rows where
/// neither buffer has been modified, turning an O(width*height) scan
/// into O(width * dirty_rows).
#[derive(Debug, Clone)]
pub struct TerminalBuffer {
    cells: Vec<Cell>,
    width: u16,
    height: u16,
    /// Per-row dirty flag. `true` means at least one cell in that row
    /// was written since the last `clear_dirty()`.
    dirty_rows: Vec<bool>,
}

impl TerminalBuffer {
    /// Create a new buffer filled with default (space) cells.
    /// All rows start dirty so the first diff produces a full comparison.
    pub fn new(width: u16, height: u16) -> Self {
        let size = (width as usize) * (height as usize);
        Self {
            cells: vec![Cell::default(); size],
            width,
            height,
            dirty_rows: vec![true; height as usize],
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// Get index into flat vec from (col, row).
    #[inline]
    fn index(&self, col: u16, row: u16) -> usize {
        (row as usize) * (self.width as usize) + (col as usize)
    }

    /// Get a cell reference at (col, row).
    pub fn get(&self, col: u16, row: u16) -> &Cell {
        let idx = self.index(col, row);
        &self.cells[idx]
    }

    /// Get a mutable cell reference at (col, row).
    ///
    /// Marks the row dirty since the caller may modify the cell.
    pub fn get_mut(&mut self, col: u16, row: u16) -> &mut Cell {
        let idx = self.index(col, row);
        self.dirty_rows[row as usize] = true;
        &mut self.cells[idx]
    }

    /// Set a cell at (col, row) and mark the row dirty.
    pub fn set(&mut self, col: u16, row: u16, cell: Cell) {
        let idx = self.index(col, row);
        self.cells[idx] = cell;
        self.dirty_rows[row as usize] = true;
    }

    /// Clear all cells to default and mark every row dirty.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.reset();
        }
        for flag in &mut self.dirty_rows {
            *flag = true;
        }
    }

    /// Resize the buffer, clearing all content. All rows are marked dirty.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let size = (width as usize) * (height as usize);
        self.cells = vec![Cell::default(); size];
        self.dirty_rows = vec![true; height as usize];
    }

    /// Return all cells as (col, row, &Cell) -- used for full redraws.
    pub fn all_cells(&self) -> Vec<(u16, u16, &Cell)> {
        let mut out = Vec::with_capacity(self.cells.len());
        for row in 0..self.height {
            for col in 0..self.width {
                let idx = self.index(col, row);
                out.push((col, row, &self.cells[idx]));
            }
        }
        out
    }

    /// Compare this buffer with another and return only changed cells.
    /// Returns (col, row, &Cell) for each cell that differs.
    ///
    /// Only rows that are dirty in *either* buffer are scanned.
    /// Clean rows are guaranteed to be identical so they are skipped entirely.
    pub fn diff<'a>(&'a self, other: &'a TerminalBuffer) -> Vec<(u16, u16, &'a Cell)> {
        assert_eq!(self.width, other.width);
        assert_eq!(self.height, other.height);

        let mut changes = Vec::new();
        for row in 0..self.height {
            let r = row as usize;
            if !self.dirty_rows[r] && !other.dirty_rows[r] {
                continue;
            }
            for col in 0..self.width {
                let idx = self.index(col, row);
                if self.cells[idx] != other.cells[idx] {
                    changes.push((col, row, &self.cells[idx]));
                }
            }
        }
        changes
    }

    /// Reset all dirty flags to `false`.
    ///
    /// Call this after flushing so that subsequent frames only scan rows
    /// that were actually written to.
    pub fn clear_dirty(&mut self) {
        for flag in &mut self.dirty_rows {
            *flag = false;
        }
    }

    /// Mark every row dirty, forcing the next `diff()` to scan all rows.
    ///
    /// Useful for a forced full redraw (e.g. after the terminal is
    /// restored from an alt-screen switch).
    pub fn mark_all_dirty(&mut self) {
        for flag in &mut self.dirty_rows {
            *flag = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::{Color, Style};

    #[test]
    fn test_new_buffer_is_spaces() {
        let buf = TerminalBuffer::new(10, 5);
        assert_eq!(buf.width(), 10);
        assert_eq!(buf.height(), 5);
        assert_eq!(buf.get(0, 0).symbol, " ");
        assert_eq!(buf.get(9, 4).symbol, " ");
    }

    #[test]
    fn test_set_and_get() {
        let mut buf = TerminalBuffer::new(10, 5);
        buf.set(3, 2, Cell::new("A"));
        assert_eq!(buf.get(3, 2).symbol, "A");
    }

    #[test]
    fn test_diff_identical() {
        let a = TerminalBuffer::new(10, 5);
        let b = TerminalBuffer::new(10, 5);
        assert!(a.diff(&b).is_empty());
    }

    #[test]
    fn test_diff_detects_changes() {
        let mut a = TerminalBuffer::new(10, 5);
        let b = TerminalBuffer::new(10, 5);

        a.set(0, 0, Cell::new("X"));
        a.set(5, 3, Cell::styled("Y", Style::default().fg(Color::Red)));

        let changes = a.diff(&b);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0], (0, 0, a.get(0, 0)));
        assert_eq!(changes[1], (5, 3, a.get(5, 3)));
    }

    #[test]
    fn test_clear() {
        let mut buf = TerminalBuffer::new(10, 5);
        buf.set(0, 0, Cell::new("X"));
        buf.clear();
        assert_eq!(buf.get(0, 0).symbol, " ");
    }
}
