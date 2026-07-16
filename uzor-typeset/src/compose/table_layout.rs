//! Two-pass column sizing (design doc §3.5): pass 1 measures every `Auto`
//! column's intrinsic content width, pass 2 distributes the region's own
//! width among `Fixed`/`Auto`/`Fraction` columns and lays out every row's
//! cells for real against the final widths.
//!
//! Divergence from the design doc's own §3.5 sketch (report, not silent):
//! the doc describes the MEASURE pass as "a recursive `compose()` call at
//! `max_width = f64::MAX`." This phase's cell content is scoped to
//! `Block::Paragraph`/`Block::Spacer` ("cell content = paragraphs" per the
//! task brief), so measuring goes straight through `uzor_text::
//! layout_paragraph` at `max_width = f64::MAX` (natural/preferred width)
//! and a near-zero width (a floor/"min" proxy — the width of the cell's
//! single widest unbreakable token) — the SAME numeric answer the doc's
//! own recursive-`compose()` approach would produce for a Paragraph cell,
//! without the overhead of standing up a full nested region-sequence
//! purely to measure a string, and it correctly handles a MULTI-run
//! `Paragraph` (several `StyledRun`s/fonts in one cell), which
//! `uzor_text::paragraph_intrinsic_size`'s single-`(text, font)` signature
//! cannot. The DISTRIBUTE pass matches the doc exactly: a real
//! `crate::compose::compose` call per cell, over a
//! [`crate::region::FixedRegionSequence`] sized to that cell's own final
//! column width.

use uzor::types::Rect;
use uzor_text::{layout_paragraph, LineShaper, Paragraph};

use super::flow::compose;
use super::ComposeStyle;
use crate::region::{FixedRegionSequence, PlacedBlock, PlacedTableCell, PlacedTableRow};
use crate::scene::{Block, BlockNode, ColumnSpec, TableBlock, TableRow};

/// One row's cells, already composed at their FINAL column widths — cached
/// across region continuations by `compose::flow` exactly like a
/// paragraph's own `ParagraphLayout` cache (measured/laid out once, at the
/// width of the region the table first entered).
pub(crate) struct ComposedRow<'a> {
    pub height: f64,
    pub cells: Vec<ComposedCell<'a>>,
}

/// One cell's already-placed content, positioned relative to the CELL's
/// own `(0, 0)` origin (translated to frame-relative coordinates only
/// when the row is actually placed — see [`place_table_rows`]).
pub(crate) struct ComposedCell<'a> {
    pub column_index: usize,
    pub blocks: Vec<PlacedBlock<'a>>,
}

/// This cell's own intrinsic `(natural, min)` width contribution — `0.0`
/// for any non-`Paragraph` content (documented scope narrowing above).
fn cell_natural_and_min_width(content: &[BlockNode<'_>], shaper: &dyn LineShaper) -> (f64, f64) {
    let mut natural = 0.0_f64;
    let mut min_width = 0.0_f64;

    for node in content {
        if let Block::Paragraph(p) = &node.kind {
            let wide = layout_paragraph(&Paragraph { max_width: f64::MAX, ..*p }, shaper);
            natural = natural.max(wide.width);
            // A near-zero max_width forces maximal wrapping — the widest
            // remaining wrapped line converges on the widest single
            // unbreakable token, our "min content width" proxy.
            let narrow = layout_paragraph(&Paragraph { max_width: 1.0, ..*p }, shaper);
            min_width = min_width.max(narrow.width);
        }
    }

    (natural, min_width)
}

/// Pass 1: resolve every column's final width for a table placed at
/// `available_width`. `Fixed` columns are unaffected; `Auto` columns get
/// exactly their widest cell's natural width when there's room, otherwise
/// shrink proportionally (never below their own min-content width);
/// `Fraction` columns split whatever's left after `Fixed` + `Auto`.
pub(crate) fn measure_table_columns(table: &TableBlock<'_>, available_width: f64, shaper: &dyn LineShaper) -> Vec<f64> {
    let n = table.columns.len();
    let mut natural = vec![0.0_f64; n];
    let mut min_width = vec![0.0_f64; n];

    for row in table.rows {
        for (col_idx, cell) in row.cells.iter().enumerate().take(n) {
            let (cell_natural, cell_min) = cell_natural_and_min_width(cell.content, shaper);
            natural[col_idx] = natural[col_idx].max(cell_natural);
            min_width[col_idx] = min_width[col_idx].max(cell_min);
        }
    }

    let mut widths = vec![0.0_f64; n];
    let mut fixed_total = 0.0_f64;
    for (i, spec) in table.columns.iter().enumerate() {
        if let ColumnSpec::Fixed(w) = spec {
            widths[i] = *w;
            fixed_total += w;
        }
    }

    let remaining = (available_width - fixed_total).max(0.0);
    let auto_idxs: Vec<usize> = table
        .columns
        .iter()
        .enumerate()
        .filter_map(|(i, s)| matches!(s, ColumnSpec::Auto).then_some(i))
        .collect();
    let auto_natural_total: f64 = auto_idxs.iter().map(|&i| natural[i]).sum();

    let auto_used = if auto_natural_total <= remaining {
        for &i in &auto_idxs {
            widths[i] = natural[i];
        }
        auto_natural_total
    } else if auto_natural_total > 0.0 {
        let scale = remaining / auto_natural_total;
        let mut used = 0.0_f64;
        for &i in &auto_idxs {
            let w = (natural[i] * scale).max(min_width[i]);
            widths[i] = w;
            used += w;
        }
        used
    } else {
        0.0
    };

    let remaining_for_fraction = (remaining - auto_used).max(0.0);
    let fraction_total: f64 = table
        .columns
        .iter()
        .filter_map(|s| if let ColumnSpec::Fraction(w) = s { Some(*w) } else { None })
        .sum();
    if fraction_total > 0.0 {
        for (i, spec) in table.columns.iter().enumerate() {
            if let ColumnSpec::Fraction(w) = spec {
                widths[i] = remaining_for_fraction * (w / fraction_total);
            }
        }
    }

    widths
}

/// Pass 2 (one row): lay out every cell for real at its own final column
/// width, via a [`FixedRegionSequence`] per cell (design doc §3.5,
/// followed verbatim for this half).
fn layout_table_row<'a>(row: &'a TableRow<'a>, column_widths: &[f64], style: &ComposeStyle, shaper: &dyn LineShaper) -> ComposedRow<'a> {
    let mut cells = Vec::with_capacity(row.cells.len());
    let mut row_height = 0.0_f64;

    for (col_idx, cell) in row.cells.iter().enumerate().take(column_widths.len()) {
        let col_width = column_widths[col_idx];
        let mut regions = FixedRegionSequence::new(Rect::new(0.0, 0.0, col_width, f64::MAX));
        let mut frames = compose(cell.content, &mut regions, style, shaper);
        let blocks = frames.pop().map(|f| f.blocks).unwrap_or_default();
        let cell_height = blocks.iter().map(|b| b.rect.bottom()).fold(0.0_f64, f64::max);
        row_height = row_height.max(cell_height);
        cells.push(ComposedCell { column_index: col_idx, blocks });
    }

    ComposedRow { height: row_height, cells }
}

/// Measure + lay out every row of `table` at `available_width` — the ONE
/// pass `compose::flow` runs when it first encounters a `Block::Table`
/// node, cached across every region the table subsequently spans (same
/// "measured once, at the width of the region it first entered"
/// convention P0 already established for `Block::Paragraph`).
pub(crate) fn measure_and_layout_table<'a>(
    table: &'a TableBlock<'a>,
    available_width: f64,
    style: &ComposeStyle,
    shaper: &dyn LineShaper,
) -> (Vec<f64>, Vec<ComposedRow<'a>>) {
    let column_widths = measure_table_columns(table, available_width, shaper);
    let rows = table.rows.iter().map(|row| layout_table_row(row, &column_widths, style, shaper)).collect();
    (column_widths, rows)
}

/// How many of `rows`, starting at `from_row`, fit within
/// `remaining_height` (summing each kept row's own `.height`) — row-atomic
/// splitting's own version of `compose::lines_fitting`, same
/// force-at-least-one-row-on-a-fresh-region degrade convention.
pub(crate) fn rows_fitting(rows: &[ComposedRow<'_>], from_row: usize, remaining_height: f64, force_at_least_one: bool) -> usize {
    let mut used = 0.0_f64;
    let mut count = 0usize;

    for row in &rows[from_row..] {
        let next = used + row.height;
        if next > remaining_height {
            break;
        }
        used = next;
        count += 1;
    }

    if count == 0 && force_at_least_one && from_row < rows.len() {
        count = 1;
    }
    count
}

/// This table's total height across EVERY row (never just what fits one
/// region) — used only by `AvoidInside`'s "would the whole table fit a
/// fresh region" pre-check and the `AvoidAfter` keep-with-next lookahead,
/// never by real per-region placement (which always goes through
/// [`rows_fitting`] instead).
pub(crate) fn table_total_height(rows: &[ComposedRow<'_>]) -> f64 {
    rows.iter().map(|r| r.height).sum()
}

/// Translate rows `[from_row, from_row + count)` of the cached `rows` into
/// frame-relative [`PlacedTableRow`]s, stacked starting at `origin`
/// (`origin.0` = the table's own left edge, `origin.1` = the cursor `y`
/// this placement starts at) — the SAME cell content
/// `layout_table_row` already computed, just shifted into place (design
/// law 1: no second position formula).
pub(crate) fn place_table_rows<'a>(
    rows: &[ComposedRow<'a>],
    from_row: usize,
    count: usize,
    column_widths: &[f64],
    origin: (f64, f64),
) -> (Vec<PlacedTableRow<'a>>, f64) {
    let (origin_x, origin_y) = origin;
    let total_width: f64 = column_widths.iter().sum();
    let mut placed_rows = Vec::with_capacity(count);
    let mut row_y = origin_y;

    for row in &rows[from_row..from_row + count] {
        let row_rect = Rect::new(origin_x, row_y, total_width, row.height);
        let mut cells = Vec::with_capacity(row.cells.len());
        let mut col_x = origin_x;

        for cell in &row.cells {
            let col_width = column_widths.get(cell.column_index).copied().unwrap_or(0.0);
            let cell_rect = Rect::new(col_x, row_y, col_width, row.height);
            let mut content: Vec<PlacedBlock<'a>> = cell.blocks.clone();
            for placed in &mut content {
                placed.translate(col_x, row_y);
            }
            cells.push(PlacedTableCell { column_index: cell.column_index, rect: cell_rect, content });
            col_x += col_width;
        }

        placed_rows.push(PlacedTableRow { rect: row_rect, cells });
        row_y += row.height;
    }

    let placed_height: f64 = rows[from_row..from_row + count].iter().map(|r| r.height).sum();
    (placed_rows, placed_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::fonts::FontFamily;
    use uzor_text::{CosmicShaper, FontSpec, StyledRun};

    use crate::scene::{TableCell, TableRow};

    fn font() -> FontSpec {
        FontSpec::new(FontFamily::Roboto, 16.0)
    }

    #[test]
    fn auto_column_gets_exactly_its_widest_cells_natural_width_when_there_is_room() {
        let shaper = CosmicShaper::headless();
        let f = font();
        let short_run = [StyledRun::new("hi", f)];
        let long_run = [StyledRun::new("a considerably longer cell value", f)];

        let short_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&short_run, f64::MAX)))];
        let long_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&long_run, f64::MAX)))];
        let empty_nodes: [BlockNode<'_>; 0] = [];

        let row1_cells = [TableCell::new(&empty_nodes), TableCell::new(&short_nodes)];
        let row2_cells = [TableCell::new(&empty_nodes), TableCell::new(&long_nodes)];
        let rows = [TableRow::new(&row1_cells), TableRow::new(&row2_cells)];
        let columns = [ColumnSpec::Fixed(40.0), ColumnSpec::Auto];
        let table = TableBlock::new(&columns, &rows);

        let expected_natural = layout_paragraph(&Paragraph::new(&long_run, f64::MAX), &shaper).width;
        let widths = measure_table_columns(&table, 600.0, &shaper);

        assert_eq!(widths[0], 40.0, "Fixed column is unaffected by measurement");
        assert!((widths[1] - expected_natural).abs() < 1e-6, "Auto column must match its widest cell's intrinsic width exactly");
    }

    #[test]
    fn auto_columns_shrink_proportionally_but_never_below_their_min_content_width_when_space_is_tight() {
        let shaper = CosmicShaper::headless();
        let f = font();
        let long_run = [StyledRun::new(
            "a considerably longer piece of cell text that wants much more room than is available",
            f,
        )];
        let nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&long_run, f64::MAX)))];
        let cells = [TableCell::new(&nodes)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Auto];
        let table = TableBlock::new(&columns, &rows);

        // Deliberately tiny budget — far less than the cell's natural width.
        let widths = measure_table_columns(&table, 30.0, &shaper);
        let (_, min_width) = cell_natural_and_min_width(&nodes, &shaper);
        assert!(widths[0] >= min_width - 1e-6, "an Auto column must never shrink below its own min-content width");
    }

    #[test]
    fn rows_fitting_matches_the_paragraph_lines_fitting_budget_convention() {
        let rows = [
            ComposedRow { height: 10.0, cells: vec![] },
            ComposedRow { height: 10.0, cells: vec![] },
            ComposedRow { height: 10.0, cells: vec![] },
        ];
        assert_eq!(rows_fitting(&rows, 0, 25.0, false), 2, "2.5 row-heights of budget fits exactly 2 whole rows");
        assert_eq!(rows_fitting(&rows, 0, 5.0, false), 0, "nothing fits and the region already has content: defer whole");
        assert_eq!(rows_fitting(&rows, 0, 5.0, true), 1, "a fresh region must make progress even if the row overflows it");
    }
}
