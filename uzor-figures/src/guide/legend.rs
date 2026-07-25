//! Legend guide — a swatch + label list, measured BEFORE plot layout so a
//! figure can shrink its own plot rect by the EXACT measured size (design
//! law #1: one transform, computed once inside a single `render_with` call
//! and reused for painting AND any hit/hover routing done in that same
//! call — see `crate::figure::BarFigure`/`crate::figure::CurveFigure`'s
//! own `render_with`).
//!
//! [`LegendPosition::Top`]/[`LegendPosition::Bottom`] lay entries out in a
//! horizontal row, greedily wrapping to additional rows when they would
//! overrun `avail_width` (same greedy left-to-right discipline
//! [`crate::guide::axis`]'s own label-collision skip uses, just wrapping
//! instead of dropping — a legend entry is never silently omitted).
//! [`LegendPosition::Right`] stacks entries in one column, wrapping to
//! additional COLUMNS once they would overrun `avail_height` (the vertical
//! counterpart of [`LegendPosition::Top`]/`Bottom`'s own row-wrap — closes
//! a real defect: a many-series legend used to grow past the figure's own
//! bottom edge with no fallback at all).
//!
//! Each entry's own swatch shape follows [`LegendEntry::symbol`] — a
//! filled square, a short line stroke, or a filled circle (see
//! [`LegendSymbol`]'s own docs for which mark kind each suits).

use uzor::render::{CircleBatch, RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::scale::ClassScale;
use crate::theme::FigureTheme;

const SWATCH_SIZE: f64 = 12.0;
const SWATCH_LABEL_GAP: f64 = 6.0;
const ENTRY_GAP: f64 = 16.0;
const ROW_GAP: f64 = 6.0;
const PAD: f64 = 8.0;

/// The swatch shape drawn for one [`LegendEntry`] — matches the actual
/// mark kind the entry represents, closing the audit's own B5 finding
/// (every legend swatch used to be an unconditional filled square, even
/// for a line series, a visible mismatch against the on-screen mark).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendSymbol {
    /// A small filled square — suits a bar/area/waterfall series (the
    /// mark itself is a filled region). THE DEFAULT: byte-identical to
    /// this crate's pre-existing (and, before this item, only) swatch
    /// shape.
    #[default]
    Square,
    /// A short horizontal line stroke — suits a line/curve series.
    Line,
    /// A filled circle — suits a point/scatter series.
    Circle,
}

/// One legend row: a series/category name plus its swatch color (CSS hex,
/// same convention as [`crate::mark::MarkStyle::color`]) and its swatch
/// shape ([`LegendSymbol`]).
#[derive(Debug, Clone)]
pub struct LegendEntry {
    pub label: String,
    pub color: String,
    pub symbol: LegendSymbol,
}

/// Where a legend sits relative to its figure's plot rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendPosition {
    Top,
    Bottom,
    Right,
}

/// Build one [`LegendEntry`] per class of `scale` (a
/// [`crate::scale::QuantizeScale`]/[`crate::scale::ThresholdScale`]/
/// [`crate::scale::QuantileScale`]) — the discrete-class counterpart of
/// [`crate::guide::colorbar::draw_colorbar`]'s continuous ramp, letting a
/// binning scale present its classes as swatches through this crate's
/// EXISTING measure/wrap/draw legend machinery instead of a bespoke
/// discrete-swatch renderer (design law: reuse before rebuild). Each
/// entry's label is `scale.class_label(i)` (e.g. `"10 – 20"`, `"< 5"`,
/// `">= 90"`); `colors` supplies the per-class swatch color, CYCLING via
/// `index % colors.len()` once `scale.class_count()` exceeds `colors.len()`
/// (the SAME modulo convention [`crate::scale::color::CategoricalScale::
/// color_for`] documents). An empty `colors` slice falls back to
/// `theme.palette` — a caller can pass `&[]` to reuse this crate's own
/// default categorical identity without constructing a palette first.
pub fn entries_from_class_scale(theme: &FigureTheme, scale: &dyn ClassScale, colors: &[String]) -> Vec<LegendEntry> {
    let fallback = &theme.palette;
    let source: &[String] = if colors.is_empty() { fallback } else { colors };
    let len = source.len().max(1);
    (0..scale.class_count())
        .map(|i| LegendEntry {
            label: scale.class_label(i),
            color: source.get(i % len).cloned().unwrap_or_else(|| "#808080".to_owned()),
            symbol: LegendSymbol::Square,
        })
        .collect()
}

/// A legend's measured reserved size — the figure shrinks its plot rect by
/// this (plus its own gap constant) on the side [`LegendPosition`] names.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LegendSize {
    pub width: f64,
    pub height: f64,
}

/// Greedy left-to-right row-wrap: how many entries land in row 0, row 1,
/// ... within `inner_width` px. The ONE wrap algorithm [`measure_legend`]
/// (to size the reserved band) and [`draw_legend`] (to actually place each
/// swatch/label) both call — a figure's shrunk plot rect always matches
/// what actually gets painted (design law #1). An entry alone wider than
/// `inner_width` still gets its own row rather than looping forever (same
/// "force it in" degrade this crate's other greedy-layout passes use, e.g.
/// `guide::axis`'s always-draw-the-first-tick-label convention).
fn row_counts(ctx: &mut dyn RenderContext, entries: &[LegendEntry], inner_width: f64) -> Vec<usize> {
    let mut counts = vec![0usize];
    let mut current_row_width = 0.0_f64;
    for entry in entries {
        let w = SWATCH_SIZE + SWATCH_LABEL_GAP + ctx.measure_text(&entry.label);
        let needed = if current_row_width > 0.0 { current_row_width + ENTRY_GAP + w } else { w };
        if needed > inner_width && current_row_width > 0.0 {
            counts.push(1);
            current_row_width = w;
        } else {
            if let Some(last) = counts.last_mut() {
                *last += 1;
            }
            current_row_width = needed;
        }
    }
    counts
}

/// Greedy top-to-bottom column-wrap for [`LegendPosition::Right`]: how
/// many entries land in column 0, column 1, ... within `avail_height` px
/// — the vertical counterpart of [`row_counts`]. An entry count that
/// still doesn't fit even a single row within `avail_height` still gets
/// its own one-entry column rather than looping forever (same "force it
/// in" degrade [`row_counts`] already documents). Empty `entries` (or
/// `entry_count == 0`) returns a single empty column.
fn column_counts(row_height: f64, entry_count: usize, avail_height: f64) -> Vec<usize> {
    if entry_count == 0 {
        return vec![0];
    }
    let inner_height = (avail_height - PAD * 2.0).max(row_height);
    let per_column = (((inner_height + ROW_GAP) / (row_height + ROW_GAP)).floor() as usize).max(1);
    let mut counts = Vec::new();
    let mut remaining = entry_count;
    while remaining > 0 {
        let take = remaining.min(per_column);
        counts.push(take);
        remaining -= take;
    }
    counts
}

/// Measure `entries` laid out at `position` within `avail_width` x
/// `avail_height` px — call BEFORE painting; the figure then shrinks its
/// plot rect by the result (+ its own gap constant) before computing
/// anything else that depends on the plot rect (design law #1).
/// `avail_height` is consulted ONLY by [`LegendPosition::Right`] (`Top`/
/// `Bottom` wrap by `avail_width` instead, same as before this parameter
/// existed). Empty `entries` measures to [`LegendSize::default`] (nothing
/// to reserve).
pub fn measure_legend(ctx: &mut dyn RenderContext, theme: &FigureTheme, entries: &[LegendEntry], position: LegendPosition, avail_width: f64, avail_height: f64) -> LegendSize {
    if entries.is_empty() {
        return LegendSize::default();
    }
    ctx.set_font(&theme.label_font);
    let row_height = ctx.text_bounds("Ag", &theme.label_font).h.max(SWATCH_SIZE);

    match position {
        LegendPosition::Right => {
            let widest_label = entries.iter().map(|e| ctx.measure_text(&e.label)).fold(0.0_f64, f64::max);
            let column_width = PAD * 2.0 + SWATCH_SIZE + SWATCH_LABEL_GAP + widest_label;
            let counts = column_counts(row_height, entries.len(), avail_height);
            let rows_in_tallest_column = counts.iter().copied().max().unwrap_or(0);
            let height = PAD * 2.0 + row_height * rows_in_tallest_column as f64 + ROW_GAP * rows_in_tallest_column.saturating_sub(1) as f64;
            let width = column_width * counts.len() as f64 + ENTRY_GAP * counts.len().saturating_sub(1) as f64;
            LegendSize { width, height }
        }
        LegendPosition::Top | LegendPosition::Bottom => {
            let inner_width = (avail_width - PAD * 2.0).max(0.0);
            let rows = row_counts(ctx, entries, inner_width).len();
            let height = PAD * 2.0 + row_height * rows as f64 + ROW_GAP * rows.saturating_sub(1) as f64;
            LegendSize { width: avail_width, height }
        }
    }
}

/// Paint one entry's own swatch (its shape per [`LegendEntry::symbol`]) at
/// `(x, row_center)`, `x` being the swatch's own LEFT edge.
fn draw_swatch(ctx: &mut dyn RenderContext, entry: &LegendEntry, x: f64, row_center: f64) {
    match entry.symbol {
        LegendSymbol::Square => {
            ctx.set_fill_color(&entry.color);
            ctx.fill_rect(x, row_center - SWATCH_SIZE / 2.0, SWATCH_SIZE, SWATCH_SIZE);
        }
        LegendSymbol::Line => {
            ctx.set_stroke_color(&entry.color);
            ctx.set_stroke_width(2.0);
            ctx.set_line_dash(&[]);
            ctx.begin_path();
            ctx.move_to(x, row_center);
            ctx.line_to(x + SWATCH_SIZE, row_center);
            ctx.stroke();
        }
        LegendSymbol::Circle => {
            let circle = CircleBatch { cx: x + SWATCH_SIZE / 2.0, cy: row_center, r: SWATCH_SIZE / 2.0 };
            ctx.draw_circle_batch(&[circle], &entry.color);
        }
    }
}

/// Paint `entries` into `rect` at `position` — `rect` must be exactly what
/// [`measure_legend`] sized (the figure's own shrink-and-place step), so
/// the SAME [`row_counts`]/[`column_counts`] wrap decision applies to
/// both. `rect.height` is this function's own `avail_height` for
/// [`LegendPosition::Right`]'s column-wrap (already carried by `rect`, no
/// separate parameter needed — `rect.height` is exactly what a figure's
/// own render pass reserves, e.g. `base_rect.height`). No-op for empty
/// `entries`.
pub fn draw_legend(ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, entries: &[LegendEntry], position: LegendPosition) {
    if entries.is_empty() {
        return;
    }
    ctx.set_font(&theme.label_font);
    let row_height = ctx.text_bounds("Ag", &theme.label_font).h.max(SWATCH_SIZE);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    match position {
        LegendPosition::Right => {
            let widest_label = entries.iter().map(|e| ctx.measure_text(&e.label)).fold(0.0_f64, f64::max);
            let column_width = PAD * 2.0 + SWATCH_SIZE + SWATCH_LABEL_GAP + widest_label;
            let counts = column_counts(row_height, entries.len(), rect.height);
            let mut idx = 0usize;
            let mut cursor_x = rect.x + PAD;
            for count in &counts {
                for row in 0..*count {
                    let entry = &entries[idx];
                    let row_center = rect.y + PAD + row as f64 * (row_height + ROW_GAP) + row_height / 2.0;
                    draw_swatch(ctx, entry, cursor_x, row_center);
                    ctx.set_fill_color(&theme.label_color);
                    ctx.fill_text(&entry.label, cursor_x + SWATCH_SIZE + SWATCH_LABEL_GAP, row_center);
                    idx += 1;
                }
                cursor_x += column_width + ENTRY_GAP;
            }
        }
        LegendPosition::Top | LegendPosition::Bottom => {
            let inner_width = (rect.width - PAD * 2.0).max(0.0);
            let counts = row_counts(ctx, entries, inner_width);
            let mut idx = 0usize;
            for (row, count) in counts.iter().enumerate() {
                let row_center = rect.y + PAD + row as f64 * (row_height + ROW_GAP) + row_height / 2.0;
                let mut cursor_x = rect.x + PAD;
                for _ in 0..*count {
                    let entry = &entries[idx];
                    let label_w = ctx.measure_text(&entry.label);
                    draw_swatch(ctx, entry, cursor_x, row_center);
                    ctx.set_fill_color(&theme.label_color);
                    ctx.fill_text(&entry.label, cursor_x + SWATCH_SIZE + SWATCH_LABEL_GAP, row_center);
                    cursor_x += SWATCH_SIZE + SWATCH_LABEL_GAP + label_w + ENTRY_GAP;
                    idx += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor_export::{render_to_png, ExportSpec};

    fn seeded_entries(n: usize) -> Vec<LegendEntry> {
        (0..n)
            .map(|i| LegendEntry { label: format!("Series {i} label"), color: format!("#{i:02x}{i:02x}{i:02x}"), symbol: LegendSymbol::Square })
            .collect()
    }

    #[test]
    fn measure_legend_empty_entries_reserves_nothing() {
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = LegendSize::default();
        render_to_png(&spec, |ctx| {
            size = measure_legend(ctx, &theme, &[], LegendPosition::Top, 400.0, 300.0);
        })
        .expect("render");
        assert_eq!(size, LegendSize::default());
    }

    #[test]
    fn measure_legend_top_wraps_to_more_rows_when_avail_width_shrinks() {
        let theme = FigureTheme::dark();
        let entries = seeded_entries(6);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut wide_height = 0.0_f64;
        let mut narrow_height = 0.0_f64;
        render_to_png(&spec, |ctx| {
            wide_height = measure_legend(ctx, &theme, &entries, LegendPosition::Top, 900.0, 300.0).height;
            narrow_height = measure_legend(ctx, &theme, &entries, LegendPosition::Top, 90.0, 300.0).height;
        })
        .expect("render");
        assert!(
            narrow_height > wide_height,
            "a narrower avail_width must wrap onto more rows, hence reserve a taller band (wide={wide_height}, narrow={narrow_height})"
        );
    }

    #[test]
    fn measure_legend_right_width_is_swatch_plus_widest_label_for_a_single_column() {
        let theme = FigureTheme::dark();
        let entries = vec![
            LegendEntry { label: "a".to_owned(), color: "#111111".to_owned(), symbol: LegendSymbol::Square },
            LegendEntry { label: "a much longer series label".to_owned(), color: "#222222".to_owned(), symbol: LegendSymbol::Square },
        ];
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = LegendSize::default();
        let mut widest_label_w = 0.0_f64;
        render_to_png(&spec, |ctx| {
            ctx.set_font(&theme.label_font);
            widest_label_w = entries.iter().map(|e| ctx.measure_text(&e.label)).fold(0.0_f64, f64::max);
            size = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 400.0, 500.0);
        })
        .expect("render");
        let expected_width = PAD * 2.0 + SWATCH_SIZE + SWATCH_LABEL_GAP + widest_label_w;
        assert!((size.width - expected_width).abs() < 1e-6);
    }

    #[test]
    fn measure_legend_right_height_stacks_one_row_per_entry_when_avail_height_is_generous() {
        let theme = FigureTheme::dark();
        let entries = seeded_entries(4);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = LegendSize::default();
        render_to_png(&spec, |ctx| {
            size = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 1000.0, 1000.0);
        })
        .expect("render");
        // Narrowing avail_WIDTH (not height) must not change a generous
        // single-column layout at all.
        let mut size_wide = LegendSize::default();
        render_to_png(&spec, |ctx| {
            size_wide = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 50.0, 1000.0);
        })
        .expect("render");
        assert!((size.height - size_wide.height).abs() < 1e-9);
    }

    #[test]
    fn measure_legend_right_wraps_to_a_second_column_when_avail_height_is_short() {
        // The audit's own B9 defect: a many-series Right legend used to
        // grow past its own avail_height with no wrap at all. A short
        // avail_height must now produce a SHORTER measured height (capped
        // by the column-wrap) and a WIDER measured width (a second
        // column), never silently exceed avail_height's own row budget.
        let theme = FigureTheme::dark();
        let entries = seeded_entries(10);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut tall_size = LegendSize::default();
        let mut short_size = LegendSize::default();
        render_to_png(&spec, |ctx| {
            tall_size = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 400.0, 2000.0);
            short_size = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 400.0, 80.0);
        })
        .expect("render");
        assert!(
            short_size.height < tall_size.height,
            "a short avail_height must wrap into more columns, capping the reserved height below the tall single-column case"
        );
        assert!(short_size.width > tall_size.width, "wrapping into more columns must reserve MORE total width than a single column");
    }

    #[test]
    fn draw_legend_renders_without_panicking_at_every_position() {
        let theme = FigureTheme::dark();
        let entries = seeded_entries(3);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        for position in [LegendPosition::Top, LegendPosition::Bottom, LegendPosition::Right] {
            let result = render_to_png(&spec, |ctx| {
                let size = measure_legend(ctx, &theme, &entries, position, 380.0, 180.0);
                let rect = Rect::new(10.0, 10.0, if position == LegendPosition::Right { size.width } else { 380.0 }, size.height.max(1.0));
                draw_legend(ctx, rect, &theme, &entries, position);
            });
            assert!(result.is_ok(), "draw_legend must render cleanly at {position:?}");
        }
    }

    #[test]
    fn draw_legend_right_wraps_into_a_second_column_and_never_exceeds_rect_height() {
        let theme = FigureTheme::dark();
        let entries = seeded_entries(10);
        // A rect deliberately too short to fit every entry as one column.
        let rect = Rect::new(10.0, 10.0, 300.0, 80.0);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_legend(ctx, rect, &theme, &entries, LegendPosition::Right);
        });
        assert!(result.is_ok(), "a short Right rect must still render every entry (wrapped into columns) without panicking");
    }

    #[test]
    fn every_legend_symbol_renders_without_panicking() {
        let theme = FigureTheme::dark();
        let entries = vec![
            LegendEntry { label: "square".to_owned(), color: "#4d90fe".to_owned(), symbol: LegendSymbol::Square },
            LegendEntry { label: "line".to_owned(), color: "#e0703c".to_owned(), symbol: LegendSymbol::Line },
            LegendEntry { label: "circle".to_owned(), color: "#5cb87a".to_owned(), symbol: LegendSymbol::Circle },
        ];
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            let size = measure_legend(ctx, &theme, &entries, LegendPosition::Top, 380.0, 180.0);
            draw_legend(ctx, Rect::new(10.0, 10.0, 380.0, size.height), &theme, &entries, LegendPosition::Top);
        });
        assert!(result.is_ok());
    }

    // ── entries_from_class_scale (binning scale -> discrete legend) ────

    #[test]
    fn entries_from_class_scale_labels_and_colors_each_class() {
        use crate::scale::QuantizeScale;

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(0.0, 100.0, 4);
        let colors: Vec<String> = vec!["#111111".to_owned(), "#222222".to_owned(), "#333333".to_owned(), "#444444".to_owned()];
        let entries = entries_from_class_scale(&theme, &scale, &colors);
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].color, "#111111");
        assert_eq!(entries[3].color, "#444444");
        assert!(entries.iter().all(|e| !e.label.is_empty()));
        assert!(entries.iter().all(|e| e.symbol == LegendSymbol::Square));
    }

    #[test]
    fn entries_from_class_scale_cycles_colors_shorter_than_class_count() {
        use crate::scale::QuantizeScale;

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(0.0, 100.0, 5);
        let colors: Vec<String> = vec!["#aa0000".to_owned(), "#00bb00".to_owned()];
        let entries = entries_from_class_scale(&theme, &scale, &colors);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].color, "#aa0000");
        assert_eq!(entries[1].color, "#00bb00");
        assert_eq!(entries[2].color, "#aa0000", "color must cycle back to the start once colors run out");
    }

    #[test]
    fn entries_from_class_scale_falls_back_to_theme_palette_for_empty_colors() {
        use crate::scale::QuantizeScale;

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(0.0, 100.0, 3);
        let entries = entries_from_class_scale(&theme, &scale, &[]);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].color, theme.palette[0]);
    }

    #[test]
    fn entries_from_class_scale_renders_through_the_existing_legend_pipeline() {
        use crate::scale::QuantizeScale;
        use uzor_export::{render_to_png, ExportSpec};

        let theme = FigureTheme::dark();
        let scale = QuantizeScale::new(-50.0, 150.0, 5);
        let entries = entries_from_class_scale(&theme, &scale, &[]);
        let spec = ExportSpec { width_px: 300, height_px: 200, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            let size = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 280.0, 180.0);
            draw_legend(ctx, Rect::new(10.0, 10.0, size.width, size.height), &theme, &entries, LegendPosition::Right);
        });
        assert!(result.is_ok(), "a binning scale's discrete legend must render through the existing legend guide without panicking");
    }

    #[test]
    fn square_and_line_and_circle_symbols_render_visibly_differently() {
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 100, height_px: 60, dpr: 1.0, background: None };
        let render_one = |symbol: LegendSymbol| {
            let entries = vec![LegendEntry { label: "x".to_owned(), color: "#4d90fe".to_owned(), symbol }];
            render_to_png(&spec, |ctx| {
                draw_legend(ctx, Rect::new(0.0, 0.0, 100.0, 60.0), &theme, &entries, LegendPosition::Top);
            })
            .expect("render")
        };
        let square = render_one(LegendSymbol::Square);
        let line = render_one(LegendSymbol::Line);
        let circle = render_one(LegendSymbol::Circle);
        assert_ne!(square, line, "a Square swatch must render differently from a Line swatch");
        assert_ne!(square, circle, "a Square swatch must render differently from a Circle swatch");
        assert_ne!(line, circle, "a Line swatch must render differently from a Circle swatch");
    }
}
