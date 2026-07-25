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
//!
//! ## Cell spanning (typography track T3 — colspan/rowspan)
//!
//! [`resolve_cell_grid`] resolves every cell's own logical grid position
//! ONCE, up front, via the same "downward-growing cells" procedure the
//! HTML table-model spec uses (simplified to this crate's own row/cell
//! shape, see that function's own doc comment) — a row's own authored
//! `cells` slice no longer maps 1:1 to grid columns once ANY cell
//! anywhere in the table declares `row_span > 1` (a row directly below a
//! rowspan omits cells for the columns it doesn't own).
//!
//! **Column-width distribution for a spanned cell** ([`grow_span_to_fit`],
//! called from [`measure_table_columns`]): every `col_span == 1` cell
//! sets its own column's natural/min floor exactly as before (unaffected
//! by spanning at all); THEN every `col_span > 1` cell (narrower spans
//! resolved first — a 2-column header grouping wins a shared column
//! before a wider 3-column one competes for it) grows its own span's
//! columns just enough that their SUM meets the spanned cell's own
//! natural/min need, distributing the DEFICIT across the span's own
//! `Auto`/`Fraction` columns ONLY (never `Fixed` — a `Fixed` column's own
//! author-declared width is exactly as immovable under a span as it
//! already is without one), proportional to each such column's OWN
//! current natural width (a column that already wants more room absorbs
//! proportionally more of the extra; every column still grows at least
//! its own fair share). This is what keeps a wide spanned cell from
//! distorting single-column widths unreasonably: a narrow column
//! participating in a wide span only grows by whatever the shortfall
//! actually requires, never force-matched to the spanning cell's own
//! full natural width. When EVERY column in a span is `Fixed` (nothing
//! eligible to grow), the deficit is simply left unresolved — the
//! spanned cell's own content wraps tighter than its natural width
//! within the fixed total, the SAME documented "shrink below natural,
//! never below min-content" degrade every other overflow case in this
//! module already uses.
//!
//! **Row-height distribution for a rowspan cell** ([`layout_table_rows`]):
//! every `row_span == 1` cell sets its own row's natural height floor
//! exactly as before; THEN every `row_span > 1` cell (again, narrower
//! spans first) that needs MORE height than the sum of the rows it
//! currently spans grows the LAST row of its own span by exactly the
//! deficit — a deliberate, documented choice (not an even/proportional
//! split across every spanned row): it keeps every row BEFORE the last
//! exactly as tall as its own singly-spanned content requires (never
//! distorted by a taller merged neighbor), the spanning cell's own top
//! edge stays exactly where it naturally started, and only the seam
//! immediately before the span releases absorbs whatever extra height
//! the merged cell needed.
//!
//! **A rowspan is atomic across a page/region break — it NEVER splits.**
//! [`rows_fitting`]/[`place_table_rows`] both resolve [`table_row_groups`]
//! (an interval-merge over every declared `row_span > 1`) and only ever
//! stop a fragment at a GROUP boundary, never inside one — the same
//! "keep-together" reasoning `compose::keep_break::BreakControl::
//! AvoidInside` already documents for a whole table, applied here at
//! rowspan-group granularity instead: a page break is never allowed to
//! fall between a merged cell's own start row and its own end row. When
//! an entire group is taller than a whole FRESH (otherwise-empty)
//! region, it still gets force-placed onto that region (the SAME "a
//! block taller than a whole fresh region overflows visibly rather than
//! looping `RegionSequence::next()` forever" degrade P0's own risk note
//! already established, now applied to a rowspan GROUP rather than a
//! single row) — the group is never torn in half across two pages either
//! way. `table_row_groups` reduces to the identity mapping
//! (`group_start == group_end == row_index`) for any table that never
//! declares `row_span > 1` anywhere, so every pre-T3 table's own
//! page-split behavior is byte-for-byte unchanged.
//!
//! **Header repeat, extended to a GROUPED header** ([`header_group_height`]
//! + `compose::flow`'s own Table arm, [`place_table_rows`]'s own
//! `repeat_header` parameter): a grouped-header table typically needs
//! TWO rows repeated together (a spanning top row + its own sub-header
//! row, e.g. `"Q1 2024"` spanning 3 columns above `"Jan"`/`"Feb"`/`"Mar"`)
//! — `TableBlock::header_repeat` therefore repeats row 0's own ATOMIC
//! ROWSPAN GROUP (via `table_row_groups(rows)[0]`), not literally just
//! `rows[0]`, at the top of every continuation fragment. For a table with
//! no rowspan on its header row at all (the overwhelming common case,
//! and every pre-T3 caller), row 0's own group is `(0, 0)` — reproducing
//! today's "repeat exactly `rows[0]`" behavior exactly.
//!
//! **Gridlines and a merged cell's own interior** (`render.rs`'s own
//! `draw_table_placement`): [`crate::region::PlacedTableRow::spans_row`]
//! is `true` for every row a rowspan either starts on or passes through —
//! `render.rs` skips that row's own (otherwise redundant) whole-row-width
//! stroke in that case, so no spurious horizontal line is ever drawn
//! through a merged cell's own interior; every individual cell's own
//! border still paints correctly via its own `stroke_rect` call either
//! way. `spans_row` is `false` for every row of a table that never uses
//! `row_span > 1` anywhere, so gridline painting is pixel-identical to
//! before T3 for every existing (non-spanning) table.

use uzor::types::Rect;
use uzor_text::{layout_paragraph, LineShaper, Paragraph};

use super::flow::compose;
use super::ComposeStyle;
use crate::region::{FixedRegionSequence, PlacedBlock, PlacedTableCell, PlacedTableRow};
use crate::scene::{Block, BlockNode, CellPadding, ColumnSpec, TableBlock};

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
    /// How many grid columns this cell's own content was composed across
    /// (typography track T3) — `1` for every pre-T3 table.
    pub col_span: usize,
    /// How many grid ROWS this cell's own content spans downward from
    /// `column_index`'s own row (typography track T3) — `1` for every
    /// pre-T3 table. This cell is attached to its own STARTING row's
    /// [`ComposedRow`] regardless of `row_span` (its `rect` is resolved
    /// to the correct merged height only at placement time, see
    /// [`place_one_row`]).
    pub row_span: usize,
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

/// One cell's resolved position in the table's own logical grid —
/// computed once, up front, from `table.rows`' own authored shape (design
/// law 1: one grid-resolution pass, consulted by BOTH the column-width
/// pass and the row-layout pass below, never re-derived twice). A row's
/// own `cells` slice is walked in author order, skipping any grid column
/// still claimed by an EARLIER row's still-active rowspan — so a cell's
/// resolved `col_start` generally is NOT the same as its own index within
/// `row.cells` once any spanning is in play (a grouped-header row's own
/// sub-header cells still line up 1:1 with `cells[i]`, since nothing
/// spans rows there; a row directly below a rowspan does not).
#[derive(Debug, Clone, Copy)]
struct GridCell {
    row_index: usize,
    cell_index: usize,
    col_start: usize,
    col_span: usize,
    row_span: usize,
}

/// Resolve every cell's own grid position. A cell whose own declared
/// `col_span` would overrun the table's own remaining column count is
/// clamped to whatever room is left (never panics); a row with more
/// cells than free grid columns simply drops the excess — both match
/// this module's own pre-existing "a short/long row degrades gracefully,
/// never panics" convention, extended to the spanning case.
fn resolve_cell_grid(table: &TableBlock<'_>) -> Vec<GridCell> {
    let n_cols = table.columns.len();
    // Exclusive: grid column `c` is free again once `row_index >=
    // occupied_until[c]`.
    let mut occupied_until = vec![0usize; n_cols];
    let mut resolved = Vec::new();

    for (row_index, row) in table.rows.iter().enumerate() {
        let mut col_cursor = 0usize;
        for (cell_index, cell) in row.cells.iter().enumerate() {
            while col_cursor < n_cols && occupied_until[col_cursor] > row_index {
                col_cursor += 1;
            }
            if col_cursor >= n_cols {
                break;
            }
            let col_span = cell.col_span.max(1).min(n_cols - col_cursor);
            let row_span = cell.row_span.max(1);
            for slot in &mut occupied_until[col_cursor..col_cursor + col_span] {
                *slot = row_index + row_span;
            }
            resolved.push(GridCell { row_index, cell_index, col_start: col_cursor, col_span, row_span });
            col_cursor += col_span;
        }
    }
    resolved
}

/// Grow `values[col_start..col_start+col_span]` so their SUM is at least
/// `needed`, distributing the deficit across only the span's own
/// non-`Fixed` columns, proportional to each one's current value (equal
/// shares when every eligible column currently reads `0.0`) — never
/// touching a `Fixed` column. A no-op when every spanned column is
/// `Fixed` (see this module's own top doc comment for the resulting
/// degrade) or when the span already meets `needed` on its own.
fn grow_span_to_fit(values: &mut [f64], columns: &[ColumnSpec], col_start: usize, col_span: usize, needed: f64) {
    let end = (col_start + col_span).min(values.len());
    if end <= col_start {
        return;
    }
    let current: f64 = values[col_start..end].iter().sum();
    let deficit = needed - current;
    if deficit <= 0.0 {
        return;
    }
    let eligible: Vec<usize> = (col_start..end).filter(|&i| !matches!(columns[i], ColumnSpec::Fixed(_))).collect();
    if eligible.is_empty() {
        return;
    }
    let eligible_total: f64 = eligible.iter().map(|&i| values[i]).sum();
    if eligible_total > 0.0 {
        for &i in &eligible {
            values[i] += deficit * (values[i] / eligible_total);
        }
    } else {
        let share = deficit / eligible.len() as f64;
        for &i in &eligible {
            values[i] += share;
        }
    }
}

/// Pass 1: resolve every column's final width for a table placed at
/// `available_width`. `Fixed` columns are unaffected; `Auto` columns get
/// exactly their widest cell's natural width (PLUS `2 * cell_padding.h`,
/// so the eventual inset content still fits without re-wrapping tighter
/// than its own natural measurement — Fix C: table cell padding) when
/// there's room, otherwise shrink proportionally (never below their own
/// min-content width, also padded); `Fraction` columns split whatever's
/// left after `Fixed` + `Auto`. Spanned cells (typography track T3) grow
/// their own span's columns first, via [`grow_span_to_fit`] — see this
/// module's own top doc comment for the exact algorithm.
pub(crate) fn measure_table_columns(table: &TableBlock<'_>, available_width: f64, shaper: &dyn LineShaper) -> Vec<f64> {
    let n = table.columns.len();
    let mut natural = vec![0.0_f64; n];
    let mut min_width = vec![0.0_f64; n];
    let pad_w = 2.0 * table.cell_padding.h;

    let grid = resolve_cell_grid(table);

    for gc in grid.iter().filter(|gc| gc.col_span == 1) {
        let cell = &table.rows[gc.row_index].cells[gc.cell_index];
        let (cell_natural, cell_min) = cell_natural_and_min_width(cell.content, shaper);
        natural[gc.col_start] = natural[gc.col_start].max(cell_natural + pad_w);
        min_width[gc.col_start] = min_width[gc.col_start].max(cell_min + pad_w);
    }

    let mut spanned: Vec<&GridCell> = grid.iter().filter(|gc| gc.col_span > 1).collect();
    spanned.sort_by_key(|gc| gc.col_span);
    for gc in spanned {
        let cell = &table.rows[gc.row_index].cells[gc.cell_index];
        let (cell_natural, cell_min) = cell_natural_and_min_width(cell.content, shaper);
        grow_span_to_fit(&mut natural, table.columns, gc.col_start, gc.col_span, cell_natural + pad_w);
        grow_span_to_fit(&mut min_width, table.columns, gc.col_start, gc.col_span, cell_min + pad_w);
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

/// For each row `r`, the `(group_start, group_end)` of the atomic
/// rowspan group it belongs to — both equal `r` itself for every row a
/// rowspan never touches (the identity mapping every pre-T3 table
/// produces for its whole `rows` slice). Rowspans that chain/overlap
/// (e.g. a cell spanning rows 0-1 and ANOTHER spanning rows 1-2) merge
/// into one contiguous group, the standard interval-merge shape.
fn table_row_groups(rows: &[ComposedRow<'_>]) -> Vec<(usize, usize)> {
    let n = rows.len();
    if n == 0 {
        return Vec::new();
    }
    let mut reach: Vec<usize> = (0..n).collect();
    for (r, row) in rows.iter().enumerate() {
        for cell in &row.cells {
            if cell.row_span > 1 {
                let end = (r + cell.row_span - 1).min(n - 1);
                reach[r] = reach[r].max(end);
            }
        }
    }

    let mut groups = vec![(0usize, 0usize); n];
    let mut row = 0usize;
    while row < n {
        let mut end = reach[row];
        let mut cursor = row;
        while cursor < end {
            cursor += 1;
            end = end.max(reach[cursor]);
        }
        for slot in groups.iter_mut().take(end + 1).skip(row) {
            *slot = (row, end);
        }
        row = end + 1;
    }
    groups
}

/// The total height of row 0's own atomic rowspan group (typography
/// track T3) — `rows[0].height` alone for every table that never uses
/// `row_span > 1` on its own header row (byte-identical to the pre-T3
/// value), or the SUM of every row a grouped header's own rowspan cell
/// reaches down through when it does. `compose::flow`'s own Table arm
/// uses this to budget [`rows_fitting`]'s `header_reserved` parameter for
/// a repeated GROUPED header, not just a single repeated row.
pub(crate) fn header_group_height(rows: &[ComposedRow<'_>]) -> f64 {
    if rows.is_empty() {
        return 0.0;
    }
    let (_, end) = table_row_groups(rows)[0];
    rows[..=end].iter().map(|r| r.height).sum()
}

/// Lay out every cell of `table` for real, at its own final SPANNED width
/// (`column_widths[col_start..col_start+col_span]`'s own sum, minus `2 *
/// padding.h`, matching Fix C's own "content is measured/wrapped at the
/// INSET width" law), via a [`FixedRegionSequence`] per cell — a
/// WHOLE-TABLE pass (not independently per row, unlike the pre-T3 code)
/// because a rowspan cell's own row-height contribution can only be
/// resolved once every row it touches is known (see this module's own
/// top doc comment for the exact rowspan-height algorithm).
fn layout_table_rows<'a>(table: &'a TableBlock<'a>, grid: &[GridCell], column_widths: &[f64], style: &ComposeStyle, shaper: &dyn LineShaper) -> Vec<ComposedRow<'a>> {
    struct Composed<'a> {
        row_index: usize,
        col_start: usize,
        col_span: usize,
        row_span: usize,
        blocks: Vec<PlacedBlock<'a>>,
        content_height: f64,
    }

    let padding = table.cell_padding;
    let n_rows = table.rows.len();

    let mut all_cells: Vec<Composed<'a>> = Vec::with_capacity(grid.len());
    for gc in grid {
        let cell = &table.rows[gc.row_index].cells[gc.cell_index];
        let end_col = (gc.col_start + gc.col_span).min(column_widths.len());
        let col_width: f64 = column_widths[gc.col_start..end_col].iter().sum();
        let content_width = (col_width - 2.0 * padding.h).max(0.0);
        let mut regions = FixedRegionSequence::new(Rect::new(0.0, 0.0, content_width, f64::MAX));
        let mut frames = compose(cell.content, &mut regions, style, shaper);
        let blocks = frames.pop().map(|f| f.blocks).unwrap_or_default();
        let content_height = blocks.iter().map(|b| b.rect.bottom()).fold(0.0_f64, f64::max);
        all_cells.push(Composed { row_index: gc.row_index, col_start: gc.col_start, col_span: gc.col_span, row_span: gc.row_span, blocks, content_height });
    }

    // Natural per-row height, from `row_span == 1` cells only — byte-
    // identical to the pre-T3 per-row max-height rule for any table that
    // never spans rows at all.
    let mut row_heights = vec![0.0_f64; n_rows];
    for c in all_cells.iter().filter(|c| c.row_span == 1) {
        row_heights[c.row_index] = row_heights[c.row_index].max(c.content_height + 2.0 * padding.v);
    }

    // Rowspan cells (narrower spans first, matching the column pass's own
    // tie-break) grow the LAST row of their own span by whatever deficit
    // remains once the naturally-sized rows are summed.
    let mut rowspans: Vec<&Composed<'a>> = all_cells.iter().filter(|c| c.row_span > 1).collect();
    rowspans.sort_by_key(|c| c.row_span);
    for c in rowspans {
        let end_row = (c.row_index + c.row_span - 1).min(n_rows.saturating_sub(1));
        let needed = c.content_height + 2.0 * padding.v;
        let current: f64 = row_heights[c.row_index..=end_row].iter().sum();
        if needed > current {
            row_heights[end_row] += needed - current;
        }
    }

    let mut rows: Vec<ComposedRow<'a>> = row_heights.into_iter().map(|height| ComposedRow { height, cells: Vec::new() }).collect();
    for c in all_cells {
        rows[c.row_index].cells.push(ComposedCell { column_index: c.col_start, col_span: c.col_span, row_span: c.row_span, blocks: c.blocks });
    }
    rows
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
    let grid = resolve_cell_grid(table);
    let rows = layout_table_rows(table, &grid, &column_widths, style, shaper);
    (column_widths, rows)
}

/// How many of `rows`, starting at `from_row`, fit within
/// `remaining_height` (summing each kept row's own `.height`) — row-atomic
/// splitting's own version of `compose::lines_fitting`, same
/// force-at-least-one-row-on-a-fresh-region degrade convention, now
/// GROUP-atomic (typography track T3): a candidate stopping point is only
/// ever the END of a [`table_row_groups`] group, so a rowspan is never
/// split across a page/region break (see this module's own top doc
/// comment). Reduces to the pre-T3 per-row loop exactly for a table that
/// never uses `row_span > 1` anywhere (`table_row_groups` there is the
/// identity mapping).
///
/// `header_reserved` (typography quality wave: table header-row repeat) is
/// subtracted from `remaining_height` BEFORE budgeting — the space a
/// repeated header row (or, since T3, a repeated header GROUP — see
/// [`header_group_height`]) would occupy at the top of this fragment,
/// `0.0` when [`crate::scene::TableBlock::header_repeat`] is off or this
/// is the table's own FIRST fragment (which already starts with the real
/// `rows[0]`, never a repeat).
pub(crate) fn rows_fitting(rows: &[ComposedRow<'_>], from_row: usize, remaining_height: f64, force_at_least_one: bool, header_reserved: f64) -> usize {
    let remaining_height = (remaining_height - header_reserved).max(0.0);
    let groups = table_row_groups(rows);
    let mut used = 0.0_f64;
    let mut count = 0usize;
    let mut row = from_row;

    while row < rows.len() {
        let (_, group_end) = groups[row];
        let group_height: f64 = rows[row..=group_end].iter().map(|r| r.height).sum();
        let next = used + group_height;
        if next > remaining_height {
            break;
        }
        used = next;
        count += group_end - row + 1;
        row = group_end + 1;
    }

    if count == 0 && force_at_least_one && from_row < rows.len() {
        let (_, group_end) = groups[from_row];
        count = group_end - from_row + 1;
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

/// Place ONE row (`rows[row_index]`) at `(origin_x, row_y)` into a
/// frame-relative [`PlacedTableRow`] — the shared per-row placement math
/// [`place_table_rows`] uses both for its ordinary row range AND (typography
/// quality wave) for a repeated header row/group, so there is exactly one
/// position formula for a placed row (design law 1), never a second copy
/// for the header-repeat case. Takes the FULL `rows` slice (not just this
/// one row) because a `row_span > 1` cell's own rect height is the SUM of
/// every row it spans downward (typography track T3) — never resolvable
/// from a single `ComposedRow` alone.
fn place_one_row<'a>(rows: &[ComposedRow<'a>], row_index: usize, column_widths: &[f64], padding: CellPadding, origin_x: f64, row_y: f64) -> PlacedTableRow<'a> {
    let row = &rows[row_index];
    let total_width: f64 = column_widths.iter().sum();
    let row_rect = Rect::new(origin_x, row_y, total_width, row.height);

    let mut col_x_offsets = Vec::with_capacity(column_widths.len());
    let mut acc = origin_x;
    for w in column_widths {
        col_x_offsets.push(acc);
        acc += w;
    }

    let mut cells = Vec::with_capacity(row.cells.len());
    // `true` for EVERY row a rowspan touches (its own start row AND every
    // row it passes THROUGH), not merely a row whose own `cells` happen
    // to include a `row_span > 1` entry — a pass-through row (e.g. a
    // grouped header's own sub-header row, still sitting beneath an
    // earlier row's rowspan) carries no such cell of its own at all, but
    // still must never receive the redundant whole-row stroke (see this
    // function's own caller-facing doc comment on `render.rs`'s
    // `draw_table_placement`). Derived from [`table_row_groups`] — a
    // group of size 1 (`group_start == group_end`) means no rowspan
    // touches this row at all.
    let (group_start, group_end_for_row) = table_row_groups(rows).get(row_index).copied().unwrap_or((row_index, row_index));
    let spans_row = group_end_for_row > group_start;

    for cell in &row.cells {
        let end_col = (cell.column_index + cell.col_span).min(column_widths.len());
        let col_x = col_x_offsets.get(cell.column_index).copied().unwrap_or(origin_x);
        let col_width: f64 = column_widths[cell.column_index..end_col].iter().sum();
        let end_row = (row_index + cell.row_span - 1).min(rows.len().saturating_sub(1));
        let cell_height: f64 = rows[row_index..=end_row].iter().map(|r| r.height).sum();

        let cell_rect = Rect::new(col_x, row_y, col_width, cell_height);
        let mut content: Vec<PlacedBlock<'a>> = cell.blocks.clone();
        for placed in &mut content {
            placed.translate(col_x + padding.h, row_y + padding.v);
        }
        cells.push(PlacedTableCell { column_index: cell.column_index, col_span: cell.col_span, row_span: cell.row_span, rect: cell_rect, content });
    }

    PlacedTableRow { rect: row_rect, cells, spans_row }
}

/// Translate rows `[from_row, from_row + count)` of the cached `rows` into
/// frame-relative [`PlacedTableRow`]s, stacked starting at `origin`
/// (`origin.0` = the table's own left edge, `origin.1` = the cursor `y`
/// this placement starts at) — the SAME cell content
/// `layout_table_rows` already computed, just shifted into place (design
/// law 1: no second position formula). `PlacedTableCell::rect` stays the
/// FULL, un-inset cell rect (gridlines paint flush against it, unchanged
/// by Fix C); the cell's own `content` is translated by `(padding.h,
/// padding.v)` off the cell's own top-left corner, INSET from it on every
/// side (already measured/wrapped at `column_width - 2*padding.h` by
/// `layout_table_rows`, so it never re-touches the gridline the way a
/// plain `(col_x, row_y)` translate would).
///
/// `repeat_header` (typography quality wave, extended by track T3) prepends
/// re-placed copies of row 0's own ATOMIC GROUP (`rows[0]` alone, or every
/// row a grouped header's own rowspan reaches through — see
/// [`table_row_groups`]/[`header_group_height`]) at the top of this
/// fragment when `from_row > 0` (a genuine continuation, never the
/// table's own first fragment, which already starts with the real
/// header) — its height is folded into the returned total, matching
/// `rows_fitting`'s own `header_reserved` budget.
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
    let header_group_end = if show_header { table_row_groups(rows)[0].1 } else { 0 };

    let mut placed_rows = Vec::with_capacity(count + if show_header { header_group_end + 1 } else { 0 });
    let mut row_y = origin_y;
    let mut placed_height = 0.0_f64;

    if show_header {
        for r in 0..=header_group_end {
            let placed = place_one_row(rows, r, column_widths, padding, origin_x, row_y);
            row_y += placed.rect.height;
            placed_height += placed.rect.height;
            placed_rows.push(placed);
        }
    }

    for r in from_row..from_row + count {
        let placed = place_one_row(rows, r, column_widths, padding, origin_x, row_y);
        row_y += placed.rect.height;
        placed_height += placed.rect.height;
        placed_rows.push(placed);
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

    // ── Typography track T3 (cell spanning) ────────────────────────────

    /// `TableCell::new` defaults `col_span`/`row_span` to `1`, and the
    /// grid resolver reproduces plain array-index column assignment for a
    /// table that never declares a span anywhere — the regression floor
    /// every OTHER T3 test builds on.
    #[test]
    fn a_cell_with_no_span_declared_resolves_to_its_own_plain_column_index() {
        let f = font();
        let a_run = [StyledRun::new("a", f)];
        let b_run = [StyledRun::new("b", f)];
        let a_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&a_run, f64::MAX)))];
        let b_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&b_run, f64::MAX)))];
        let cells = [TableCell::new(&a_nodes), TableCell::new(&b_nodes)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Fixed(50.0), ColumnSpec::Fixed(50.0)];
        let table = TableBlock::new(&columns, &rows);

        let grid = resolve_cell_grid(&table);
        assert_eq!(grid.len(), 2);
        assert_eq!(grid[0].col_start, 0);
        assert_eq!(grid[1].col_start, 1);
        assert!(grid.iter().all(|g| g.col_span == 1 && g.row_span == 1));
    }

    /// A cell directly below a `row_span: 2` cell correctly skips the
    /// grid column the rowspan still owns, landing on the NEXT free
    /// column instead of colliding with it — the "downward-growing
    /// cells" mechanism's own core case.
    #[test]
    fn a_cell_below_an_active_rowspan_skips_the_still_occupied_column() {
        let f = font();
        let tall_run = [StyledRun::new("tall", f)];
        let tall_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&tall_run, f64::MAX)))];
        let side_run = [StyledRun::new("side", f)];
        let side_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&side_run, f64::MAX)))];
        let below_run = [StyledRun::new("below", f)];
        let below_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&below_run, f64::MAX)))];

        let row0_cells = [TableCell::new(&tall_nodes).with_row_span(2), TableCell::new(&side_nodes)];
        let row1_cells = [TableCell::new(&below_nodes)];
        let rows = [TableRow::new(&row0_cells), TableRow::new(&row1_cells)];
        let columns = [ColumnSpec::Fixed(50.0), ColumnSpec::Fixed(50.0)];
        let table = TableBlock::new(&columns, &rows);

        let grid = resolve_cell_grid(&table);
        assert_eq!(grid.len(), 3);
        let row1_cell = grid.iter().find(|g| g.row_index == 1).expect("row 1's own cell must resolve");
        assert_eq!(row1_cell.col_start, 1, "row 1's own cell must land on column 1 (column 0 is still claimed by the rowspan)");
    }

    /// Grouped-header case: a header cell spanning 3 columns grows those
    /// 3 columns' combined width to meet its own need, distributing the
    /// deficit across them WITHOUT ever collapsing any one of the 3
    /// sub-columns below what a normal (non-spanning) sub-header cell in
    /// the SAME column would need on its own.
    #[test]
    fn a_colspan_header_grows_its_own_spanned_columns_without_starving_any_one_sub_column() {
        let f = font();
        let group_run = [StyledRun::new("A considerably wide quarterly grouping header label", f)];
        let group_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&group_run, f64::MAX)))];
        let jan_run = [StyledRun::new("Jan", f)];
        let feb_run = [StyledRun::new("Feb", f)];
        let mar_run = [StyledRun::new("Mar", f)];
        let jan_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&jan_run, f64::MAX)))];
        let feb_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&feb_run, f64::MAX)))];
        let mar_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&mar_run, f64::MAX)))];

        let header_row_cells = [TableCell::new(&group_nodes).with_col_span(3)];
        let sub_row_cells = [TableCell::new(&jan_nodes), TableCell::new(&feb_nodes), TableCell::new(&mar_nodes)];
        let rows = [TableRow::new(&header_row_cells), TableRow::new(&sub_row_cells)];
        let columns = [ColumnSpec::Auto, ColumnSpec::Auto, ColumnSpec::Auto];
        let table = TableBlock::new(&columns, &rows);
        let shaper = CosmicShaper::headless();

        // Baseline: what each sub-column would need with NO grouped
        // header at all (a plain 3-column table of just the sub-header
        // row) — the floor no spanning distribution may ever push a
        // column below.
        let baseline_rows = [TableRow::new(&sub_row_cells)];
        let baseline_table = TableBlock::new(&columns, &baseline_rows);
        let baseline_widths = measure_table_columns(&baseline_table, 2000.0, &shaper);

        let widths = measure_table_columns(&table, 2000.0, &shaper);
        assert_eq!(widths.len(), 3);
        for i in 0..3 {
            assert!(widths[i] >= baseline_widths[i] - 1e-6, "column {i} must never shrink below its own non-spanned floor because of a grouped header");
        }
        let spanned_total: f64 = widths.iter().sum();
        let (group_natural, _) = cell_natural_and_min_width(&group_nodes, &shaper);
        assert!(spanned_total >= group_natural + 2.0 * table.cell_padding.h - 1e-6, "the 3 spanned columns' combined width must meet the group header's own natural need");
    }

    /// A `Fixed` column inside a colspan is never resized by the span —
    /// the deficit lands entirely on the OTHER (non-`Fixed`) column(s) in
    /// the same span.
    #[test]
    fn a_fixed_column_inside_a_colspan_is_never_resized_the_deficit_lands_on_the_other_column() {
        let f = font();
        let wide_run = [StyledRun::new("A deliberately very wide spanning header label indeed", f)];
        let wide_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&wide_run, f64::MAX)))];
        let cells = [TableCell::new(&wide_nodes).with_col_span(2)];
        let rows = [TableRow::new(&cells)];
        let columns = [ColumnSpec::Fixed(30.0), ColumnSpec::Auto];
        let table = TableBlock::new(&columns, &rows);
        let shaper = CosmicShaper::headless();

        let widths = measure_table_columns(&table, 2000.0, &shaper);
        assert_eq!(widths[0], 30.0, "the Fixed column must stay exactly at its own declared width, span or no span");
        let (wide_natural, _) = cell_natural_and_min_width(&wide_nodes, &shaper);
        assert!(widths[0] + widths[1] >= wide_natural + 2.0 * table.cell_padding.h - 1e-6, "the Auto column alone must absorb the whole deficit");
    }

    /// A rowspan cell taller than the sum of the rows it naturally spans
    /// grows exactly the LAST row of its own span by the deficit — the
    /// row(s) before the last stay at their own naturally-sized height.
    #[test]
    fn a_tall_rowspan_cell_grows_only_the_last_row_of_its_own_span() {
        let f = font();
        let tall_run = [StyledRun::new(
            "A rowspan cell with several lines of body content that add up to noticeably more height than two ordinary short single-line rows would naturally need on their own.",
            f,
        )];
        let tall_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&tall_run, 120.0)))];
        let short_a_run = [StyledRun::new("a", f)];
        let short_b_run = [StyledRun::new("b", f)];
        let short_a_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&short_a_run, f64::MAX)))];
        let short_b_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&short_b_run, f64::MAX)))];

        let row0_cells = [TableCell::new(&tall_nodes).with_row_span(2), TableCell::new(&short_a_nodes)];
        let row1_cells = [TableCell::new(&short_b_nodes)];
        let rows = [TableRow::new(&row0_cells), TableRow::new(&row1_cells)];
        let columns = [ColumnSpec::Fixed(120.0), ColumnSpec::Fixed(80.0)];
        let table = TableBlock::new(&columns, &rows);
        let style = ComposeStyle::new(0.0, f);
        let shaper = CosmicShaper::headless();

        let (_, composed_rows) = measure_and_layout_table(&table, 200.0, &style, &shaper);
        assert_eq!(composed_rows.len(), 2);

        // Row 0's own natural floor is governed by its `row_span == 1`
        // sibling cell (`short_a`) alone — independently compose that
        // SAME content at column 1's own resolved width (where `short_a`
        // actually sits) to learn what row 0's height must equal if the
        // rowspan cell contributes nothing to it.
        let (column_widths, _) = measure_and_layout_table(&table, 200.0, &style, &shaper);
        let short_a_content_width = (column_widths[1] - 2.0 * table.cell_padding.h).max(0.0);
        let short_a_layout = crate::compose::compose(
            &short_a_nodes,
            &mut crate::region::FixedRegionSequence::new(uzor::types::Rect::new(0.0, 0.0, short_a_content_width, f64::MAX)),
            &style,
            &shaper,
        );
        let short_a_content_height: f64 = short_a_layout[0].blocks.iter().map(|b| b.rect.bottom()).fold(0.0, f64::max);
        let row0_natural_alone = short_a_content_height + 2.0 * table.cell_padding.v;
        assert!((composed_rows[0].height - row0_natural_alone).abs() < 1e-6, "row 0 must keep its own naturally-sized height, unmodified by the taller rowspan neighbor");

        let combined = composed_rows[0].height + composed_rows[1].height;
        let tall_content_width = (column_widths[0] - 2.0 * table.cell_padding.h).max(0.0);
        let tall_layout = crate::compose::compose(
            &tall_nodes,
            &mut crate::region::FixedRegionSequence::new(uzor::types::Rect::new(0.0, 0.0, tall_content_width, f64::MAX)),
            &style,
            &shaper,
        );
        let tall_content_height: f64 = tall_layout[0].blocks.iter().map(|b| b.rect.bottom()).fold(0.0, f64::max);
        assert!(tall_content_height > 0.0, "fixture's tall cell must actually wrap to real content height");
        assert!(combined >= tall_content_height + 2.0 * table.cell_padding.v - 1e-6, "the two rows combined must be at least tall enough for the rowspan cell's own content");
        assert!(composed_rows[1].height > row0_natural_alone, "row 1 (the LAST row of the span) must be the one that grew to absorb the deficit, taller than row 0's own natural floor");
    }

    /// A `row_span: 2` cell's own placed rect height equals the SUM of
    /// the two rows it spans — not just the first row's own height.
    #[test]
    fn a_placed_rowspan_cells_rect_height_equals_the_sum_of_its_spanned_rows() {
        let f = font();
        let tall_run = [StyledRun::new("merged", f)];
        let tall_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&tall_run, f64::MAX)))];
        let side_a_run = [StyledRun::new("side text that wraps across a couple of lines to force row 0 taller", f)];
        let side_a_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&side_a_run, 60.0)))];
        let side_b_run = [StyledRun::new("b", f)];
        let side_b_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&side_b_run, f64::MAX)))];

        let row0_cells = [TableCell::new(&tall_nodes).with_row_span(2), TableCell::new(&side_a_nodes)];
        let row1_cells = [TableCell::new(&side_b_nodes)];
        let rows = [TableRow::new(&row0_cells), TableRow::new(&row1_cells)];
        let columns = [ColumnSpec::Fixed(120.0), ColumnSpec::Fixed(70.0)];
        let table = TableBlock::new(&columns, &rows);
        let style = ComposeStyle::new(0.0, f);
        let shaper = CosmicShaper::headless();

        let (column_widths, composed_rows) = measure_and_layout_table(&table, 200.0, &style, &shaper);
        let (placed, total_height) = place_table_rows(&composed_rows, 0, 2, &column_widths, table.cell_padding, (0.0, 0.0), false);
        assert_eq!(placed.len(), 2);

        let merged_cell = placed[0].cells.iter().find(|c| c.row_span == 2).expect("row 0 must carry the rowspan cell");
        let expected_height = composed_rows[0].height + composed_rows[1].height;
        assert!((merged_cell.rect.height - expected_height).abs() < 1e-6, "a rowspan-2 cell's own rect height must equal the sum of both spanned rows");
        assert!((total_height - expected_height).abs() < 1e-6);

        // The row directly below the rowspan carries no cell at column 0
        // (it's still claimed by the merged cell above) — its own single
        // cell resolves to column 1.
        assert_eq!(placed[1].cells.len(), 1);
        assert_eq!(placed[1].cells[0].column_index, 1);
    }

    /// A rowspan is NEVER split across a page/region break: a table whose
    /// naive per-row budget would land the split INSIDE a `row_span: 2`
    /// cell's own range instead stops the fragment BEFORE that rowspan
    /// starts, deferring the whole merged group to the next region.
    #[test]
    fn rows_fitting_never_splits_a_page_break_inside_an_active_rowspan() {
        let single_height = 10.0;
        let rows = [
            ComposedRow { height: single_height, cells: vec![] }, // row 0: plain
            ComposedRow {
                height: single_height,
                cells: vec![ComposedCell { column_index: 0, col_span: 1, row_span: 2, blocks: vec![] }],
            }, // row 1: starts a rowspan reaching into row 2
            ComposedRow { height: single_height, cells: vec![] }, // row 2: end of the rowspan
            ComposedRow { height: single_height, cells: vec![] }, // row 3: plain
        ];

        // A budget of 2.5 row-heights would naively fit rows [0, 1] (25
        // units), landing the split RIGHT INSIDE the rowspan (row 1
        // starts it, row 2 finishes it) — group-atomicity must instead
        // stop at row 0 only (10 units), deferring the WHOLE 2-row group
        // (rows 1-2) to the next region.
        let count = rows_fitting(&rows, 0, single_height * 2.5, false, 0.0);
        assert_eq!(count, 1, "the split must land BEFORE the rowspan group starts, never inside it");

        // A budget of exactly 3.5 row-heights (35 units) comfortably fits
        // the whole group (rows 0-2, 30 units) plus room to spare, but
        // NOT row 3 too — must include the complete group, never a
        // partial one.
        let count2 = rows_fitting(&rows, 0, single_height * 3.5, false, 0.0);
        assert_eq!(count2, 3, "a budget covering the WHOLE rowspan group must include every row of it, never a partial group");

        // Degenerate case: a budget too small even for the plain row 0
        // alone, on a fresh (otherwise-empty) region — the SAME
        // force-at-least-one degrade as an ordinary oversized single row,
        // now applied to the group starting at `from_row` when `from_row`
        // itself is inside a rowspan's own start.
        let count3 = rows_fitting(&rows, 1, single_height * 0.5, true, 0.0);
        assert_eq!(count3, 2, "a fresh region must still force-place the WHOLE rowspan group rather than splitting it, even when it overflows");
    }

    /// Grouped-header repeat: `TableBlock::header_repeat` on a table whose
    /// OWN row 0 is part of a 2-row rowspan group (a spanning top label
    /// above per-column sub-headers) repeats BOTH rows of that group at
    /// the top of a continuation fragment, not just `rows[0]` alone.
    #[test]
    fn header_repeat_repeats_the_whole_grouped_header_not_just_row_zero() {
        let f = font();
        let group_run = [StyledRun::new("Group", f)];
        let group_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&group_run, f64::MAX)))];
        let sub_a_run = [StyledRun::new("A", f)];
        let sub_b_run = [StyledRun::new("B", f)];
        let sub_a_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&sub_a_run, f64::MAX)))];
        let sub_b_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&sub_b_run, f64::MAX)))];
        let body_run = [StyledRun::new("cell", f)];
        let body_nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&body_run, f64::MAX)))];

        // Row 0: a rowspan-2 label in column 0 (spans down into row 1)
        // plus a colspan-2 group title across columns 1-2. Row 1: the
        // per-column sub-headers for columns 1-2 (column 0 is still
        // claimed by row 0's own rowspan). Rows 2+: plain body rows.
        let header_row0_cells = [TableCell::new(&group_nodes).with_row_span(2), TableCell::new(&group_nodes).with_col_span(2)];
        let header_row1_cells = [TableCell::new(&sub_a_nodes), TableCell::new(&sub_b_nodes)];
        let body_cells = [TableCell::new(&body_nodes), TableCell::new(&body_nodes), TableCell::new(&body_nodes)];

        let mut rows: Vec<TableRow<'_>> = vec![TableRow::new(&header_row0_cells), TableRow::new(&header_row1_cells)];
        for _ in 0..6 {
            rows.push(TableRow::new(&body_cells));
        }
        let columns = [ColumnSpec::Auto, ColumnSpec::Auto, ColumnSpec::Auto];
        let table = TableBlock::new(&columns, &rows).with_header_repeat(true);
        let style = ComposeStyle::new(0.0, f);
        let shaper = CosmicShaper::headless();

        let (column_widths, composed_rows) = measure_and_layout_table(&table, 300.0, &style, &shaper);
        assert_eq!(header_group_height(&composed_rows), composed_rows[0].height + composed_rows[1].height, "the header group's own reserved height must cover BOTH header rows, not just row 0");

        // Place a continuation fragment starting mid-body (from_row = 3)
        // and confirm the repeated header carries exactly 2 rows.
        let (placed, placed_height) = place_table_rows(&composed_rows, 3, 2, &column_widths, table.cell_padding, (0.0, 0.0), true);
        // 2 repeated header rows + 2 real body rows.
        assert_eq!(placed.len(), 4, "the continuation must repeat BOTH header rows, then continue with its own body rows");
        let repeated_row0_text: String = placed[0].cells[0]
            .content
            .iter()
            .filter_map(|b| b.paragraph_layout.as_ref())
            .flat_map(|l| l.glyphs.iter().map(|g| g.cluster.as_str()))
            .collect();
        assert_eq!(repeated_row0_text, "Group", "the repeated group carries the same header text as the real one");
        let repeated_row1_text: String = placed[1].cells[0]
            .content
            .iter()
            .filter_map(|b| b.paragraph_layout.as_ref())
            .flat_map(|l| l.glyphs.iter().map(|g| g.cluster.as_str()))
            .collect();
        assert_eq!(repeated_row1_text, "A", "the repeated sub-header row must also be present, not just row 0");

        let header_height = header_group_height(&composed_rows);
        assert!((placed_height - (header_height + composed_rows[3].height + composed_rows[4].height)).abs() < 1e-6);

        // `spans_row` marks both rows of the repeated group (a rowspan
        // passes through both) — never the two plain body rows after it.
        assert!(placed[0].spans_row, "row 0 of the repeated header (the rowspan's own start) must be marked spans_row");
        assert!(placed[1].spans_row, "row 1 of the repeated header (the rowspan passing through it) must also be marked spans_row");
        assert!(!placed[2].spans_row && !placed[3].spans_row, "the plain body rows after the header must never be marked spans_row");
    }

    /// `spans_row` is `false` on every row of a table that never declares
    /// `row_span > 1` anywhere — the byte-identical-gridline-painting
    /// regression floor for every pre-T3 table.
    #[test]
    fn spans_row_is_false_everywhere_for_a_table_with_no_rowspan_at_all() {
        let f = font();
        let run = [StyledRun::new("plain", f)];
        let nodes = [BlockNode::new(Block::Paragraph(Paragraph::new(&run, f64::MAX)))];
        let cells = [TableCell::new(&nodes), TableCell::new(&nodes)];
        let rows = [TableRow::new(&cells), TableRow::new(&cells)];
        let columns = [ColumnSpec::Fixed(50.0), ColumnSpec::Fixed(50.0)];
        let table = TableBlock::new(&columns, &rows);
        let style = ComposeStyle::new(0.0, f);
        let shaper = CosmicShaper::headless();

        let (column_widths, composed_rows) = measure_and_layout_table(&table, 200.0, &style, &shaper);
        let (placed, _) = place_table_rows(&composed_rows, 0, 2, &column_widths, table.cell_padding, (0.0, 0.0), false);
        assert!(placed.iter().all(|r| !r.spans_row));
    }
}
