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
//! [`LegendPosition::Right`] stacks entries in one column, one per row,
//! sized to the widest label — it does not itself wrap by height (no
//! `avail_height` is threaded through this guide; a caller with a hard
//! height budget is a later concern no current figure needs).
//!
//! Every entry's swatch is a small filled square (never a line sample) —
//! one shape shared by every figure kind (`BarFigure`/`CurveFigure`) keeps
//! this guide free of per-figure-kind branching; a curve series' legend
//! swatch reads perfectly well as "this series' own color," same as a bar
//! series' swatch does.

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::theme::FigureTheme;

const SWATCH_SIZE: f64 = 12.0;
const SWATCH_LABEL_GAP: f64 = 6.0;
const ENTRY_GAP: f64 = 16.0;
const ROW_GAP: f64 = 6.0;
const PAD: f64 = 8.0;

/// One legend row: a series/category name plus its swatch color (CSS hex,
/// same convention as [`crate::mark::MarkStyle::color`]).
#[derive(Debug, Clone)]
pub struct LegendEntry {
    pub label: String,
    pub color: String,
}

/// Where a legend sits relative to its figure's plot rect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendPosition {
    Top,
    Bottom,
    Right,
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

/// Measure `entries` laid out at `position` within `avail_width` px — call
/// BEFORE painting; the figure then shrinks its plot rect by the result
/// (+ its own gap constant) before computing anything else that depends on
/// the plot rect (design law #1). Empty `entries` measures to
/// [`LegendSize::default`] (nothing to reserve).
pub fn measure_legend(ctx: &mut dyn RenderContext, theme: &FigureTheme, entries: &[LegendEntry], position: LegendPosition, avail_width: f64) -> LegendSize {
    if entries.is_empty() {
        return LegendSize::default();
    }
    ctx.set_font(&theme.label_font);
    let row_height = ctx.text_bounds("Ag", &theme.label_font).h.max(SWATCH_SIZE);

    match position {
        LegendPosition::Right => {
            let widest_label = entries.iter().map(|e| ctx.measure_text(&e.label)).fold(0.0_f64, f64::max);
            let width = PAD * 2.0 + SWATCH_SIZE + SWATCH_LABEL_GAP + widest_label;
            let rows = entries.len();
            let height = PAD * 2.0 + row_height * rows as f64 + ROW_GAP * rows.saturating_sub(1) as f64;
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

/// Paint `entries` into `rect` at `position` — `rect` must be exactly what
/// [`measure_legend`] sized (the figure's own shrink-and-place step), so
/// the SAME [`row_counts`] wrap decision applies to both. No-op for empty
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
            for (i, entry) in entries.iter().enumerate() {
                let row_center = rect.y + PAD + i as f64 * (row_height + ROW_GAP) + row_height / 2.0;
                ctx.set_fill_color(&entry.color);
                ctx.fill_rect(rect.x + PAD, row_center - SWATCH_SIZE / 2.0, SWATCH_SIZE, SWATCH_SIZE);
                ctx.set_fill_color(&theme.label_color);
                ctx.fill_text(&entry.label, rect.x + PAD + SWATCH_SIZE + SWATCH_LABEL_GAP, row_center);
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
                    ctx.set_fill_color(&entry.color);
                    ctx.fill_rect(cursor_x, row_center - SWATCH_SIZE / 2.0, SWATCH_SIZE, SWATCH_SIZE);
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
        (0..n).map(|i| LegendEntry { label: format!("Series {i} label"), color: format!("#{i:02x}{i:02x}{i:02x}") }).collect()
    }

    #[test]
    fn measure_legend_empty_entries_reserves_nothing() {
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = LegendSize::default();
        render_to_png(&spec, |ctx| {
            size = measure_legend(ctx, &theme, &[], LegendPosition::Top, 400.0);
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
            wide_height = measure_legend(ctx, &theme, &entries, LegendPosition::Top, 900.0).height;
            narrow_height = measure_legend(ctx, &theme, &entries, LegendPosition::Top, 90.0).height;
        })
        .expect("render");
        assert!(
            narrow_height > wide_height,
            "a narrower avail_width must wrap onto more rows, hence reserve a taller band (wide={wide_height}, narrow={narrow_height})"
        );
    }

    #[test]
    fn measure_legend_right_width_is_swatch_plus_widest_label() {
        let theme = FigureTheme::dark();
        let entries = vec![
            LegendEntry { label: "a".to_owned(), color: "#111111".to_owned() },
            LegendEntry { label: "a much longer series label".to_owned(), color: "#222222".to_owned() },
        ];
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = LegendSize::default();
        let mut widest_label_w = 0.0_f64;
        render_to_png(&spec, |ctx| {
            ctx.set_font(&theme.label_font);
            widest_label_w = entries.iter().map(|e| ctx.measure_text(&e.label)).fold(0.0_f64, f64::max);
            size = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 400.0);
        })
        .expect("render");
        let expected_width = PAD * 2.0 + SWATCH_SIZE + SWATCH_LABEL_GAP + widest_label_w;
        assert!((size.width - expected_width).abs() < 1e-6);
    }

    #[test]
    fn measure_legend_right_height_stacks_one_row_per_entry_never_wrapping() {
        let theme = FigureTheme::dark();
        let entries = seeded_entries(4);
        let spec = ExportSpec { width_px: 10, height_px: 10, dpr: 1.0, background: None };
        let mut size = LegendSize::default();
        render_to_png(&spec, |ctx| {
            size = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 50.0);
        })
        .expect("render");
        // A Right legend never wraps by width — narrowing `avail_width`
        // must not change its (entries-count-driven) height at all.
        let mut size_wide = LegendSize::default();
        render_to_png(&spec, |ctx| {
            size_wide = measure_legend(ctx, &theme, &entries, LegendPosition::Right, 1000.0);
        })
        .expect("render");
        assert!((size.height - size_wide.height).abs() < 1e-9);
    }

    #[test]
    fn draw_legend_renders_without_panicking_at_every_position() {
        let theme = FigureTheme::dark();
        let entries = seeded_entries(3);
        let spec = ExportSpec { width_px: 400, height_px: 200, dpr: 1.0, background: None };
        for position in [LegendPosition::Top, LegendPosition::Bottom, LegendPosition::Right] {
            let result = render_to_png(&spec, |ctx| {
                let size = measure_legend(ctx, &theme, &entries, position, 380.0);
                let rect = Rect::new(10.0, 10.0, if position == LegendPosition::Right { size.width } else { 380.0 }, size.height.max(1.0));
                draw_legend(ctx, rect, &theme, &entries, position);
            });
            assert!(result.is_ok(), "draw_legend must render cleanly at {position:?}");
        }
    }
}
