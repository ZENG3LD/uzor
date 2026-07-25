//! [`TableBlock`] — two-pass column sizing table (design doc §3.5), now
//! with cell spanning (typography track T3 — grouped headers/merged
//! cells): [`TableCell::col_span`]/[`TableCell::row_span`], the two
//! fields P1's own divergence log deliberately left out ("a future phase
//! that needs spanning gains them non-breaking ... once a real
//! span-aware layout consumer needs them" — this is that phase).
//! `col_span`/`row_span` both default to `1` via [`TableCell::new`], so
//! every pre-T3 call site (none of which ever calls
//! [`TableCell::with_col_span`]/[`TableCell::with_row_span`]) is
//! byte-for-byte unaffected — see `compose::table_layout`'s own module
//! docs for the grid-resolution/width-distribution/rowspan-height
//! algorithm this enables.
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
    /// How many logical grid columns this cell covers, starting at
    /// whatever column [`crate::compose::table_layout`]'s own grid
    /// resolution assigns it (never a literal array index once spanning
    /// cells are involved — see that module's own doc comment). `1` (the
    /// default, set by [`TableCell::new`]) is the pre-T3 "one cell, one
    /// column" behavior, unchanged. `0` is treated as `1` (a builder
    /// input floor, never a fallible surface).
    pub col_span: usize,
    /// How many logical grid ROWS this cell covers downward from the row
    /// it's authored on. `1` (the default) is the pre-T3 behavior. `0` is
    /// treated as `1`, same floor as `col_span`.
    pub row_span: usize,
}

impl<'a> TableCell<'a> {
    pub fn new(content: &'a [BlockNode<'a>]) -> Self {
        Self { content, col_span: 1, row_span: 1 }
    }

    /// Builder: this cell covers `col_span` logical grid columns
    /// (grouped-header case, e.g. a header cell spanning 3 sub-columns).
    pub fn with_col_span(mut self, col_span: usize) -> Self {
        self.col_span = col_span.max(1);
        self
    }

    /// Builder: this cell covers `row_span` logical grid rows downward
    /// (merged-cell case). See `compose::table_layout`'s own module doc
    /// for how a rowspan crossing a page break is handled (it never does
    /// — a rowspan is atomic across region/page boundaries, exactly like
    /// this crate's existing row-atomic table splitting already treats a
    /// single row).
    pub fn with_row_span(mut self, row_span: usize) -> Self {
        self.row_span = row_span.max(1);
        self
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
    /// When `true` and this table splits between rows across regions,
    /// `rows[0]` (this table's own first/header row) re-renders at the top
    /// of every CONTINUATION fragment (never the first fragment, which
    /// already starts with it) — its height is counted in
    /// `compose::table_layout::rows_fitting`'s own budget for that
    /// continuation, so a repeated header never causes a fragment to
    /// overflow its region. Default `false` (additive — every pre-existing
    /// table is byte-identical). See [`TableBlock::with_header_repeat`].
    pub header_repeat: bool,
}

impl<'a> TableBlock<'a> {
    pub fn new(columns: &'a [ColumnSpec], rows: &'a [TableRow<'a>]) -> Self {
        Self { columns, rows, cell_padding: CellPadding::default(), header_repeat: false }
    }

    /// Builder: override the default per-cell content inset.
    pub fn with_cell_padding(mut self, cell_padding: CellPadding) -> Self {
        self.cell_padding = cell_padding;
        self
    }

    /// Builder: opt into repeating `rows[0]` at the top of every
    /// continuation fragment when this table splits across regions.
    pub fn with_header_repeat(mut self, header_repeat: bool) -> Self {
        self.header_repeat = header_repeat;
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
