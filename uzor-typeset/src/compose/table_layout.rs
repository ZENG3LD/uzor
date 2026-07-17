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
use crate::scene::{Block, BlockNode, CellPadding, ColumnSpec, TableBlock, TableRow};

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
/// exactly their widest cell's natural width (PLUS `2 * cell_padding.h`,
/// so the eventual inset content still fits without re-wrapping tighter
/// than its own natural measurement — Fix C: table cell padding) when
/// there's room, otherwise shrink proportionally (never below their own
/// min-content width, also padded); `Fraction` columns split whatever's
/// left after `Fixed` + `Auto`.
pub(crate) fn measure_table_columns(table: &TableBlock<'_>, available_width: f64, shaper: &dyn LineShaper) -> Vec<f64> {
    let n = table.columns.len();
    let mut natural = vec![0.0_f64; n];
    let mut min_width = vec![0.0_f64; n];
    let pad_w = 2.0 * table.cell_padding.h;

    for row in table.rows {
        for (col_idx, cell) in row.cells.iter().enumerate().take(n) {
            let (cell_natural, cell_min) = cell_natural_and_min_width(cell.content, shaper);
            natural[col_idx] = natural[col_idx].max(cell_natural + pad_w);
            min_width[col_idx] = min_width[col_idx].max(cell_min + pad_w);
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
/// width MINUS `2 * padding.h` (Fix C: content is measured/wrapped at the
/// INSET width, never the full column width, so it never touches the
/// eventual gridline), via a [`FixedRegionSequence`] per cell (design doc
/// §3.5, followed verbatim for this half, padding aside). The row's own
/// height is the tallest cell's content height plus `2 * padding.v`.
fn layout_table_row<'a>(row: &'a TableRow<'a>, column_widths: &[f64], padding: CellPadding, style: &ComposeStyle, shaper: &dyn LineShaper) -> ComposedRow<'a> {
    let mut cells = Vec::with_capacity(row.cells.len());
    let mut row_height = 0.0_f64;

    for (col_idx, cell) in row.cells.iter().enumerate().take(column_widths.len()) {
        let col_width = column_widths[col_idx];
        let content_width = (col_width - 2.0 * padding.h).max(0.0);
        let mut regions = FixedRegionSequence::new(Rect::new(0.0, 0.0, content_width, f64::MAX));
        let mut frames = compose(cell.content, &mut regions, style, shaper);
        let blocks = frames.pop().map(|f| f.blocks).unwrap_or_default();
        let cell_content_height = blocks.iter().map(|b| b.rect.bottom()).fold(0.0_f64, f64::max);
        row_height = row_height.max(cell_content_height + 2.0 * padding.v);
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
    let rows = table.rows.iter().map(|row| layout_table_row(row, &column_widths, table.cell_padding, style, shaper)).collect();
    (column_widths, rows)
}

/// How many of `rows`, starting at `from_row`, fit within
/// `remaining_height` (summing each kept row's own `.height`) — row-atomic
/// splitting's own version of `compose::lines_fitting`, same
/// force-at-least-one-row-on-a-fresh-region degrade convention.
///
/// `header_reserved` (typography quality wave: table header-row repeat) is
/// subtracted from `remaining_height` BEFORE budgeting — the space a
/// repeated header row would occupy at the top of this fragment, `0.0`
/// when [`crate::scene::TableBlock::header_repeat`] is off or this is the
/// table's own FIRST fragment (which already starts with the real
/// `rows[0]`, never a repeat).
pub(crate) fn rows_fitting(rows: &[ComposedRow<'_>], from_row: usize, remaining_height: f64, force_at_least_one: bool, header_reserved: f64) -> usize {
    let remaining_height = (remaining_height - header_reserved).max(0.0);
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

/// Place ONE `row` at `(origin_x, row_y)` into a frame-relative
/// [`PlacedTableRow`] — the shared per-row placement math [`place_table_rows`]
/// uses both for its ordinary row range AND (typography quality wave) for a
/// repeated header row, so there is exactly one position formula for a
/// placed row (design law 1), never a second copy for the header-repeat
/// case.
fn place_one_row<'a>(row: &ComposedRow<'a>, column_widths: &[f64], padding: CellPadding, origin_x: f64, row_y: f64) -> PlacedTableRow<'a> {
    let total_width: f64 = column_widths.iter().sum();
    let row_rect = Rect::new(origin_x, row_y, total_width, row.height);
    let mut cells = Vec::with_capacity(row.cells.len());
    let mut col_x = origin_x;

    for cell in &row.cells {
        let col_width = column_widths.get(cell.column_index).copied().unwrap_or(0.0);
        let cell_rect = Rect::new(col_x, row_y, col_width, row.height);
        let mut content: Vec<PlacedBlock<'a>> = cell.blocks.clone();
        for placed in &mut content {
            placed.translate(col_x + padding.h, row_y + padding.v);
        }
        cells.push(PlacedTableCell { column_index: cell.column_index, rect: cell_rect, content });
        col_x += col_width;
    }

    PlacedTableRow { rect: row_rect, cells }
}

/// Translate rows `[from_row, from_row + count)` of the cached `rows` into
/// frame-relative [`PlacedTableRow`]s, stacked starting at `origin`
/// (`origin.0` = the table's own left edge, `origin.1` = the cursor `y`
/// this placement starts at) — the SAME cell content
/// `layout_table_row` already computed, just shifted into place (design
/// law 1: no second position formula). `PlacedTableCell::rect` stays the
/// FULL, un-inset cell rect (gridlines paint flush against it, unchanged
/// by Fix C); the cell's own `content` is translated by `(padding.h,
/// padding.v)` off the cell's own top-left corner, INSET from it on every
/// side (already measured/wrapped at `column_width - 2*padding.h` by
/// `layout_table_row`, so it never re-touches the gridline the way a
/// plain `(col_x, row_y)` translate would).
///
/// `repeat_header` (typography quality wave) prepends a re-placed copy of
/// `rows[0]` at the top of this fragment when `from_row > 0` (a genuine
/// continuation, never the table's own first fragment, which already
/// starts with the real header row) — its height is folded into the
/// returned total, matching `rows_fitting`'s own `header_reserved` budget.
pub(crate) fn place_table_rows<'a>(
    rows: &[ComposedRow<'a>],
    from_row: usize,
    count: usize,
    column_widths: &[f64],
    padding: CellPadding,
    origin: (f64, f64),
    repeat_header: bool,
) -> (Vec<PlacedTableRow<'a>>, f64) {
    let (origin_x, origin_y) = origin;
    let show_header = repeat_header && from_row > 0 && !rows.is_empty();
    let mut placed_rows = Vec::with_capacity(count + show_header as usize);
    let mut row_y = origin_y;
    let mut placed_height = 0.0_f64;

    if show_header {
        placed_rows.push(place_one_row(&rows[0], column_widths, padding, origin_x, row_y));
        row_y += rows[0].height;
        placed_height += rows[0].height;
    }

    for row in &rows[from_row..from_row + count] {
        placed_rows.push(place_one_row(row, column_widths, padding, origin_x, row_y));
        row_y += row.height;
        placed_height += row.height;
    }

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
        // Fix C: an Auto column's measured width is its widest cell's own
        // intrinsic width PLUS `2 * cell_padding.h` (the default inset),
        // never the bare content width alone.
        let expected_padded = expected_natural + 2.0 * table.cell_padding.h;
        assert!(
            (widths[1] - expected_padded).abs() < 1e-6,
            "Auto column must match its widest cell's intrinsic width plus 2x cell padding exactly"
        );
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

    /// Fix C proof: a placed cell's own CONTENT rect (its paragraph's
    /// `PlacedBlock::rect`) sits inset from the cell's own `rect` by
    /// exactly `cell_padding` on every side — never flush against the
    /// gridline the un-inset `cell.rect` is drawn at.
    #[test]
    fn placed_cell_content_rect_is_inset_from_the_cell_rect_on_every_side() {
        let shaper = CosmicShaper::headless();
        let f = font();
        let style = ComposeStyle::new(0.0, f);

        let cell_run = [StyledRun::new("padded cell text", f)];
        let cell_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&cell_run, f64::MAX)))];
        let cells = [TableCell::new(&cell_nodes)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Fixed(200.0)];
        let table = TableBlock::new(&columns, &rows);
        let padding = table.cell_padding;

        let (column_widths, composed_rows) = measure_and_layout_table(&table, 200.0, &style, &shaper);
        let (placed_rows, _) = place_table_rows(&composed_rows, 0, 1, &column_widths, padding, (0.0, 0.0), false);

        let cell = &placed_rows[0].cells[0];
        assert!(!cell.content.is_empty(), "fixture cell must have placed content to check the inset against");

        let content_left = cell.content.iter().map(|b| b.rect.x).fold(f64::MAX, f64::min);
        let content_top = cell.content.iter().map(|b| b.rect.y).fold(f64::MAX, f64::min);
        let content_right = cell.content.iter().map(|b| b.rect.x + b.rect.width).fold(0.0_f64, f64::max);
        let content_bottom = cell.content.iter().map(|b| b.rect.bottom()).fold(0.0_f64, f64::max);

        assert!((content_left - (cell.rect.x + padding.h)).abs() < 1e-6, "content left edge must be inset by cell_padding.h");
        assert!((content_top - (cell.rect.y + padding.v)).abs() < 1e-6, "content top edge must be inset by cell_padding.v");
        assert!(
            (cell.rect.x + cell.rect.width - content_right - padding.h).abs() < 1e-6,
            "content right edge must be inset by cell_padding.h"
        );
        assert!(
            (cell.rect.y + cell.rect.height - content_bottom - padding.v).abs() < 1e-6,
            "content bottom edge must be inset by cell_padding.v"
        );
    }

    #[test]
    fn rows_fitting_matches_the_paragraph_lines_fitting_budget_convention() {
        let rows = [
            ComposedRow { height: 10.0, cells: vec![] },
            ComposedRow { height: 10.0, cells: vec![] },
            ComposedRow { height: 10.0, cells: vec![] },
        ];
        assert_eq!(rows_fitting(&rows, 0, 25.0, false, 0.0), 2, "2.5 row-heights of budget fits exactly 2 whole rows");
        assert_eq!(rows_fitting(&rows, 0, 5.0, false, 0.0), 0, "nothing fits and the region already has content: defer whole");
        assert_eq!(rows_fitting(&rows, 0, 5.0, true, 0.0), 1, "a fresh region must make progress even if the row overflows it");
    }

    #[test]
    fn rows_fitting_subtracts_the_reserved_header_height_from_the_budget() {
        let rows = [
            ComposedRow { height: 10.0, cells: vec![] },
            ComposedRow { height: 10.0, cells: vec![] },
            ComposedRow { height: 10.0, cells: vec![] },
        ];
        // Same 25.0 budget as above, but 10.0 reserved for a repeated
        // header row — only 1 whole body row fits, not 2.
        assert_eq!(rows_fitting(&rows, 0, 25.0, false, 10.0), 1, "the reserved header height must come out of the budget before counting body rows");
    }

    /// Header-row repeat (typography quality wave): a 10-row table split
    /// across 2 regions must repeat `rows[0]` (the header) at the TOP of
    /// the continuation fragment (never the first fragment, which already
    /// starts with the real header), and the budget for that continuation
    /// must account for the repeated header's own height.
    #[test]
    fn header_row_repeats_at_the_top_of_a_continuation_fragment_and_its_height_is_budgeted() {
        let shaper = CosmicShaper::headless();
        let f = font();
        let style = ComposeStyle::new(0.0, f);

        let header_run = [StyledRun::new("Header", f)];
        let header_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&header_run, f64::MAX)))];
        let header_cells = [TableCell::new(&header_nodes)];

        let body_run = [StyledRun::new("cell", f)];
        let body_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, f64::MAX)))];
        let body_cells = [TableCell::new(&body_nodes)];

        let rows: Vec<TableRow<'_>> =
            std::iter::once(TableRow::new(&header_cells)).chain((0..9).map(|_| TableRow::new(&body_cells))).collect();
        let columns = [ColumnSpec::Auto];
        let table = TableBlock::new(&columns, &rows).with_header_repeat(true);

        let (column_widths, composed_rows) = measure_and_layout_table(&table, 200.0, &style, &shaper);
        let row_height = composed_rows[0].height;
        assert!(composed_rows.iter().all(|r| (r.height - row_height).abs() < 1e-6), "fixture rows must be equal height");

        // Budget room for exactly 4 rows on the FIRST fragment (header +
        // 3 body rows), no repeat reserved yet (from_row == 0).
        let first_count = rows_fitting(&composed_rows, 0, row_height * 4.0 + 0.5, false, 0.0);
        assert_eq!(first_count, 4, "the first fragment carries the real header + 3 body rows, no reservation needed");

        // The continuation fragment starts at row 4 (0-indexed) — budget
        // room for exactly 3 more rows PLUS the repeated header's own
        // height (4 rows' worth of space, but only 3 are real body rows).
        let header_height = composed_rows[0].height;
        let continuation_count = rows_fitting(&composed_rows, first_count, row_height * 4.0 + 0.5, false, header_height);
        assert_eq!(continuation_count, 3, "the repeated header's own height must come out of the continuation's budget");

        let (placed, placed_height) =
            place_table_rows(&composed_rows, first_count, continuation_count, &column_widths, table.cell_padding, (0.0, 0.0), true);

        assert_eq!(placed.len(), continuation_count + 1, "the continuation fragment must carry the repeated header PLUS its own body rows");
        let repeated_header_text: String = placed[0].cells[0]
            .content
            .iter()
            .filter_map(|b| b.paragraph_layout.as_ref())
            .flat_map(|l| l.glyphs.iter().map(|g| g.cluster.as_str()))
            .collect();
        assert_eq!(repeated_header_text, "Header", "the continuation's own first placed row must be the repeated header content");
        assert!(
            (placed_height - (header_height + row_height * continuation_count as f64)).abs() < 1e-6,
            "the returned total height must include the repeated header's own height"
        );

        // Sanity: the FIRST fragment (from_row == 0) never repeats a
        // header even when `header_repeat` is on — it already starts with
        // the real one.
        let (first_placed, _) = place_table_rows(&composed_rows, 0, first_count, &column_widths, table.cell_padding, (0.0, 0.0), true);
        assert_eq!(first_placed.len(), first_count, "the first fragment must never ALSO get a repeated header on top of the real one");
    }
}
