//! [`TableBlock`] — two-pass column sizing table (design doc §3.5).
//!
//! P1 scope narrowing (report, not silent): the design doc's own
//! `TableCell` sketch carries `col_span`/`row_span` fields; this phase's
//! task brief scopes tables to plain grids only (no spanning), and per
//! this crate's "no field nothing reads" convention (matches P0's own
//! divergence notes), those two fields are NOT added here — a future
//! phase that needs spanning gains them non-breaking (a builder default
//! of `1` keeps every P1 call site unaffected) once a real span-aware
//! layout consumer needs them.
//!
//! Cell content is `&'a [BlockNode<'a>]` (paragraphs this phase, per the
//! task brief — a cell CAN hold any block kind the type allows, but only
//! `Block::Paragraph`/`Block::Spacer` contribute to this phase's column
//! measurement/row height math; see `compose::table_layout`'s own module
//! docs for exactly what's measured).

use crate::scene::block::BlockNode;

/// One column's width policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnSpec {
    /// A fixed width, never resized by the distribute pass.
    Fixed(f64),
    /// Sized to its widest cell's own intrinsic content width, subject to
    /// the distribute pass's available-width budget (design doc §3.5).
    Auto,
    /// A share of whatever width remains after `Fixed` + `Auto` columns
    /// are satisfied, weighted by this `f64`.
    Fraction(f64),
}

/// One table cell.
pub struct TableCell<'a> {
    pub content: &'a [BlockNode<'a>],
}

impl<'a> TableCell<'a> {
    pub fn new(content: &'a [BlockNode<'a>]) -> Self {
        Self { content }
    }
}

/// One table row — `cells.len()` is expected to match `TableBlock::columns.len()`;
/// a short row (fewer cells than columns) simply leaves its trailing
/// columns blank for that row, a long row's extra cells are ignored (both
/// degrade gracefully rather than panicking, matching
/// `uzor-figures::BarFigure`'s own "shorter of the two" convention for a
/// `categories`/`values` length mismatch).
pub struct TableRow<'a> {
    pub cells: &'a [TableCell<'a>],
}

impl<'a> TableRow<'a> {
    pub fn new(cells: &'a [TableCell<'a>]) -> Self {
        Self { cells }
    }
}

/// Horizontal + vertical inset applied INSIDE every cell, on all four
/// sides — content never composes/paints flush against a cell's own
/// gridlines (readability defect fix: cramped tables). Applied in BOTH
/// `compose::table_layout`'s measure pass (an `Auto` column's intrinsic
/// width grows by `2*h` to make room for the inset) and its placement
/// pass (each cell's content is composed at `column_width - 2*h` and
/// translated by `(h, v)` off the cell's own top-left corner) — gridlines
/// themselves are drawn at the UN-inset cell rect (`render::
/// draw_table_placement` is unchanged), only the CONTENT rect insets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellPadding {
    pub h: f64,
    pub v: f64,
}

impl CellPadding {
    pub const fn new(h: f64, v: f64) -> Self {
        Self { h, v }
    }
}

impl Default for CellPadding {
    /// ~6px horizontal / ~4px vertical — a sensible v1 default so cell
    /// content is never composed flush against a gridline out of the box.
    fn default() -> Self {
        Self { h: 6.0, v: 4.0 }
    }
}

/// A flow-participating table: two-pass column sizing (design doc §3.5),
/// row-atomic splitting across regions (a row never splits mid-row; the
/// table splits BETWEEN rows when it spans more than one region — see
/// `compose::table_layout`).
pub struct TableBlock<'a> {
    pub columns: &'a [ColumnSpec],
    pub rows: &'a [TableRow<'a>],
    /// Per-cell content inset — [`CellPadding::default`] unless overridden
    /// via [`TableBlock::with_cell_padding`].
    pub cell_padding: CellPadding,
}

impl<'a> TableBlock<'a> {
    pub fn new(columns: &'a [ColumnSpec], rows: &'a [TableRow<'a>]) -> Self {
        Self { columns, rows, cell_padding: CellPadding::default() }
    }

    /// Builder: override the default per-cell content inset.
    pub fn with_cell_padding(mut self, cell_padding: CellPadding) -> Self {
        self.cell_padding = cell_padding;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_block_carries_its_columns_and_rows_verbatim() {
        let columns = [ColumnSpec::Fixed(40.0), ColumnSpec::Auto, ColumnSpec::Fraction(1.0)];
        let cells: [TableCell<'_>; 0] = [];
        let rows = [TableRow::new(&cells)];
        let table = TableBlock::new(&columns, &rows);
        assert_eq!(table.columns.len(), 3);
        assert_eq!(table.rows.len(), 1);
    }
}
