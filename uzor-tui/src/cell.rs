//! Terminal cell: single character + style.
//!
//! Uses `CompactString` instead of `String` for the symbol field to avoid
//! heap allocation. Terminal cell symbols are almost always 1-4 bytes
//! (single ASCII or small Unicode), which `CompactString` stores inline
//! (up to ~24 bytes on 64-bit). This eliminates ~3840 heap allocations
//! for a typical 80x24 double-buffered terminal.

use compact_str::CompactString;
use crate::style::Style;

/// A single terminal cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The grapheme displayed in this cell (usually 1 char).
    ///
    /// Stored as `CompactString` to avoid heap allocation for short symbols.
    pub symbol: CompactString,
    /// Visual style.
    pub style: Style,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            symbol: CompactString::const_new(" "),
            style: Style::default(),
        }
    }
}

impl Cell {
    /// Create a cell with a symbol and default style.
    pub fn new(symbol: impl Into<CompactString>) -> Self {
        Self {
            symbol: symbol.into(),
            style: Style::default(),
        }
    }

    /// Create a cell with symbol and style.
    pub fn styled(symbol: impl Into<CompactString>, style: Style) -> Self {
        Self {
            symbol: symbol.into(),
            style,
        }
    }

    /// Reset to default (space with no style).
    pub fn reset(&mut self) {
        self.symbol = CompactString::const_new(" ");
        self.style = Style::default();
    }
}
