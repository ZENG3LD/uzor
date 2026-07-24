//! `HeatmapFigure` — a `BandScale` x `BandScale` grid, cell fill driven by
//! a [`ColorScale`] (sequential by default, or diverging via
//! [`HeatmapFigure::with_diverging`]), with a [`crate::guide::colorbar`]
//! legend.
//!
//! Cells tile the plot rect EXACTLY — both band scales use `padding =
//! 0.0` (a heatmap reads as a contiguous grid, unlike a bar chart's
//! deliberately-gapped bands) — via [`layout_heatmap`], the same
//! pure-layout-separate-from-painting split every other figure in this
//! crate uses (design law #1); [`hit_test_cell`] hit-tests through that
//! SAME geometry.
//!
//! Row labels read top-down in natural input order via
//! [`crate::coord::PlotArea::y_band`] (the SAME non-inverted convention
//! [`crate::figure::TimelineFigure`]'s lane rows already use — a
//! categorical row axis has no "larger value plots higher" continuous-
//! scale semantics to invert), painted by a small custom pass rather than
//! [`crate::guide::axis::draw_y_axis`] (which assumes the OPPOSITE,
//! inverted convention). Column labels DO reuse
//! [`crate::guide::axis::draw_x_axis`] unchanged — `BandScale::map`'s
//! index-order center is already left-to-right, so no inversion mismatch
//! exists on that axis.

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
use crate::guide::{axis, colorbar, tooltip};
use crate::mark::text::draw_label_centered;
use crate::scale::color::ColorScale;
use crate::scale::linear::format_value;
use crate::scale::BandScale;
use crate::theme::FigureTheme;

const MARGIN_LEFT: f64 = 70.0;
const MARGIN_BOTTOM: f64 = 28.0;
const TITLE_HEIGHT: f64 = 24.0;
const LABEL_GAP: f64 = 6.0;
const COLORBAR_GAP: f64 = 12.0;
const CELL_LABEL_PAD: f64 = 6.0;
/// A cell's own value label draws in whichever of black/white contrasts
/// more with its own fill — thresholded on the fill's OKLCH lightness via
/// a cheap sRGB luma proxy (good enough for a "is this cell dark or
/// light" binary choice, no perceptual color math needed for that).
const CELL_LABEL_LIGHT: &str = "#ffffff";
const CELL_LABEL_DARK: &str = "#101216";
const HOVER_STROKE_WIDTH: f64 = 2.0;

/// One resolved grid cell — pure geometry + (optional, missing/non-finite
/// input skips) value, same index-carrying convention
/// [`crate::figure::sankey::RibbonGeom`] uses (geometry separate from the
/// caller's own data, painting reads the rest back through the index).
#[derive(Debug, Clone, Copy)]
pub struct HeatmapCell {
    pub row: usize,
    pub col: usize,
    pub rect: Rect,
    pub value: Option<f64>,
}

/// Pure layout geometry for a [`HeatmapFigure`] — see the module docs.
#[derive(Debug, Clone)]
pub struct HeatmapLayout {
    pub cells: Vec<HeatmapCell>,
    pub x_band: BandScale,
    pub y_band: BandScale,
}

/// Lay `x_labels` x `y_labels` out as a zero-padding (tight-tiling) grid
/// inside `area`'s own rect. `values[row][col]` missing or non-finite
/// entries still get a full-size cell rect (the grid always tiles
/// exactly) but carry `value: None` (painted as a gap, no fill/label).
pub fn layout_heatmap(x_labels: &[String], y_labels: &[String], values: &[Vec<f64>], area: &PlotArea) -> HeatmapLayout {
    let x_band = BandScale::new(x_labels.to_vec(), 0.0);
    let y_band = BandScale::new(y_labels.to_vec(), 0.0);

    let mut cells = Vec::with_capacity(x_labels.len() * y_labels.len());
    for row in 0..y_labels.len() {
        let (top, bottom) = area.y_band(&y_band, row);
        for col in 0..x_labels.len() {
            let (left, right) = area.x_band(&x_band, col);
            let value = values.get(row).and_then(|r| r.get(col)).copied().filter(|v| v.is_finite());
            cells.push(HeatmapCell { row, col, rect: Rect::new(left, top, (right - left).max(0.0), (bottom - top).max(0.0)), value });
        }
    }
    HeatmapLayout { cells, x_band, y_band }
}

/// Index of the cell whose rect contains `(px, py)` — hit-tests through
/// the EXACT SAME `cells` geometry [`layout_heatmap`] produced (design
/// law #1).
pub fn hit_test_cell(layout: &HeatmapLayout, px: f64, py: f64) -> Option<usize> {
    layout.cells.iter().position(|c| c.rect.contains(px, py))
}

/// Cheap perceptual-enough "is this fill dark or light" luma proxy over a
/// `"#rrggbb"`/`"#rrggbbaa"` hex string — good enough to pick a readable
/// black/white label color, not a color-science claim.
fn is_dark_fill(hex: &str) -> bool {
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return true;
    }
    let Ok(r) = u8::from_str_radix(&hex[0..2], 16) else { return true };
    let Ok(g) = u8::from_str_radix(&hex[2..4], 16) else { return true };
    let Ok(b) = u8::from_str_radix(&hex[4..6], 16) else { return true };
    let luma = 0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64;
    luma < 140.0
}

#[derive(Debug, Clone, Copy)]
enum ColorMode {
    Sequential,
    Diverging { mid: f64 },
}

/// A `BandScale` x `BandScale` heatmap grid with a continuous-color
/// [`crate::guide::colorbar`] legend.
pub struct HeatmapFigure {
    pub x_labels: Vec<String>,
    pub y_labels: Vec<String>,
    pub values: Vec<Vec<f64>>,
    color_mode: ColorMode,
    title: Option<String>,
}

impl HeatmapFigure {
    /// `values[row][col]` — `row` indexes `y_labels`, `col` indexes
    /// `x_labels`. Defaults to a sequential color scale over the data's
    /// own finite extent.
    pub fn new(x_labels: Vec<String>, y_labels: Vec<String>, values: Vec<Vec<f64>>) -> Self {
        Self { x_labels, y_labels, values, color_mode: ColorMode::Sequential, title: None }
    }

    /// Switch to a diverging color scale around `mid` (e.g. `0.0` for a
    /// signed-delta grid) instead of the default sequential ramp.
    pub fn with_diverging(mut self, mid: f64) -> Self {
        self.color_mode = ColorMode::Diverging { mid };
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn data_extent(&self) -> (f64, f64) {
        self.values.iter().flatten().copied().filter(|v| v.is_finite()).fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), v| (mn.min(v), mx.max(v)))
    }

    /// This figure's own color scale — sequential (default) or diverging
    /// ([`HeatmapFigure::with_diverging`]) over the data's finite extent.
    /// Exposed so a caller building the SAME colorbar externally (or a
    /// test asserting a known cell's own color) reads through the exact
    /// scale this figure paints with.
    pub fn color_scale(&self) -> ColorScale {
        let (mn, mx) = self.data_extent();
        let (mn, mx) = if mn.is_finite() && mx.is_finite() { (mn, mx) } else { (0.0, 1.0) };
        match self.color_mode {
            ColorMode::Sequential => ColorScale::sequential(mn, mx),
            ColorMode::Diverging { mid } => ColorScale::diverging(mn, mid, mx),
        }
    }

    fn base_plot_rect(&self, rect: Rect) -> Rect {
        let title_h = if self.title.is_some() { TITLE_HEIGHT } else { 0.0 };
        Rect::new(
            rect.x + MARGIN_LEFT,
            rect.y + title_h,
            (rect.width - MARGIN_LEFT).max(0.0),
            (rect.height - title_h - MARGIN_BOTTOM).max(0.0),
        )
    }

    /// This figure's plot rect for `rect` — same "does NOT account for the
    /// colorbar" caveat as every other figure's ctx-less `plot_rect`/
    /// `plot_area` accessor (measuring the colorbar's label width needs
    /// live text metrics); `render_with` shrinks the SAME base rect
    /// internally via its own `ctx`.
    pub fn plot_rect(&self, rect: Rect) -> Rect {
        self.base_plot_rect(rect)
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`, reacting to `overlay`'s
    /// borrowed per-frame interaction state: a hover position over a cell
    /// draws a highlight border and an x/y/value tooltip.
    /// `overlay.focus`/`overlay.brush` are not consumed by this figure.
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        if self.x_labels.is_empty() || self.y_labels.is_empty() {
            if let Some(title) = &self.title {
                crate::figure::draw_title(ctx, rect, title, theme);
            }
            return;
        }

        let scale = self.color_scale();
        let base_rect = self.base_plot_rect(rect);
        let colorbar_size = colorbar::measure_colorbar(ctx, theme, &scale);
        let reserved = colorbar_size.width + COLORBAR_GAP;
        let plot_rect = Rect::new(base_rect.x, base_rect.y, (base_rect.width - reserved).max(0.0), base_rect.height);
        let colorbar_rect = Rect::new(base_rect.right() - colorbar_size.width, base_rect.y, colorbar_size.width, base_rect.height);

        let area = PlotArea::new(plot_rect);
        let layout = layout_heatmap(&self.x_labels, &self.y_labels, &self.values, &area);

        for cell in &layout.cells {
            if cell.rect.width <= 0.0 || cell.rect.height <= 0.0 {
                continue;
            }
            let Some(value) = cell.value else { continue };
            let hex = scale.color_at(value);
            ctx.set_fill_color(&hex);
            ctx.fill_rect(cell.rect.x, cell.rect.y, cell.rect.width, cell.rect.height);

            let text = format_value(value, 1.0);
            ctx.set_font(&theme.label_font);
            let text_w = ctx.measure_text(&text);
            let text_h = ctx.text_bounds(&text, &theme.label_font).h;
            if text_w + CELL_LABEL_PAD <= cell.rect.width && text_h + CELL_LABEL_PAD <= cell.rect.height {
                let label_color = if is_dark_fill(&hex) { CELL_LABEL_LIGHT } else { CELL_LABEL_DARK };
                draw_label_centered(ctx, &text, cell.rect.center_x(), cell.rect.center_y(), label_color, &theme.label_font);
            }
        }

        if let Some((hx, hy)) = overlay.hover_px {
            if let Some(i) = hit_test_cell(&layout, hx, hy) {
                if let Some(cell) = layout.cells.get(i) {
                    ctx.set_stroke_color(&theme.highlight);
                    ctx.set_stroke_width(HOVER_STROKE_WIDTH);
                    ctx.stroke_rect(cell.rect.x, cell.rect.y, cell.rect.width, cell.rect.height);

                    let x_label = self.x_labels.get(cell.col).cloned().unwrap_or_default();
                    let y_label = self.y_labels.get(cell.row).cloned().unwrap_or_default();
                    let value_text = cell.value.map(|v| format_value(v, 1.0)).unwrap_or_else(|| "n/a".to_owned());
                    let lines = vec![("x".to_owned(), x_label), ("y".to_owned(), y_label), ("value".to_owned(), value_text)];
                    tooltip::draw_tooltip(ctx, theme, (hx, hy), &lines, plot_rect);
                }
            }
        }

        axis::draw_x_axis(ctx, &area, &layout.x_band, theme, layout.x_band.len());
        draw_row_labels(ctx, &area, &layout.y_band, &self.y_labels, theme);
        colorbar::draw_colorbar(ctx, colorbar_rect, theme, &scale);

        if let Some(title) = &self.title {
            crate::figure::draw_title(ctx, rect, title, theme);
        }
    }
}

/// Row labels read top-down in natural input order via
/// [`PlotArea::y_band`] — see the module docs for why
/// [`crate::guide::axis::draw_y_axis`] (inverted-value convention) doesn't
/// apply here.
fn draw_row_labels(ctx: &mut dyn RenderContext, area: &PlotArea, y_band: &BandScale, y_labels: &[String], theme: &FigureTheme) {
    ctx.set_font(&theme.label_font);
    ctx.set_text_align(TextAlign::Right);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_fill_color(&theme.label_color);
    for (i, label) in y_labels.iter().enumerate() {
        let (top, bottom) = area.y_band(y_band, i);
        let cy = (top + bottom) / 2.0;
        ctx.fill_text(label, area.rect.x - LABEL_GAP, cy);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(n: usize, prefix: &str) -> Vec<String> {
        (0..n).map(|i| format!("{prefix}{i}")).collect()
    }

    #[test]
    fn cell_rects_tile_the_plot_exactly_adjacent_edges_abut() {
        let x_labels = labels(4, "x");
        let y_labels = labels(3, "y");
        let values = vec![vec![1.0; 4]; 3];
        let area = PlotArea::new(Rect::new(10.0, 20.0, 400.0, 300.0));
        let layout = layout_heatmap(&x_labels, &y_labels, &values, &area);

        // Total covered area exactly matches the plot rect's own area (no
        // gaps, no overlaps) — a strong tiling proof independent of any
        // one row/col pair.
        let total_area: f64 = layout.cells.iter().map(|c| c.rect.width * c.rect.height).sum();
        assert!((total_area - area.rect.width * area.rect.height).abs() < 1e-6);

        // Adjacent columns in the same row abut exactly.
        let get = |row: usize, col: usize| layout.cells.iter().find(|c| c.row == row && c.col == col).expect("cell exists").rect;
        for row in 0..3 {
            for col in 0..3 {
                let left = get(row, col);
                let right = get(row, col + 1);
                assert!((left.right() - right.x).abs() < 1e-9, "row {row} col {col}/{}: must abut with no gap", col + 1);
            }
        }
        // Adjacent rows in the same column abut exactly, top-down.
        for row in 0..2 {
            for col in 0..4 {
                let top_cell = get(row, col);
                let bottom_cell = get(row + 1, col);
                assert!((top_cell.bottom() - bottom_cell.y).abs() < 1e-9, "row {row}/{}: must abut with no gap", row + 1);
            }
        }
        // Row 0 sits at the very top (natural top-down order).
        assert!((get(0, 0).y - area.rect.y).abs() < 1e-9);
    }

    #[test]
    fn color_of_a_known_cell_matches_an_independently_built_scale() {
        let x_labels = labels(2, "x");
        let y_labels = labels(2, "y");
        let values = vec![vec![0.0, 50.0], vec![100.0, 25.0]];
        let figure = HeatmapFigure::new(x_labels.clone(), y_labels.clone(), values.clone());

        // Data extent is [0, 100] — verify the figure's own exposed scale
        // uses exactly that domain (not the caller's guess).
        let (mn, mx) = figure.color_scale().domain();
        assert!((mn - 0.0).abs() < 1e-9);
        assert!((mx - 100.0).abs() < 1e-9);

        // The known cell (row 0, col 1) has value 50 — its fill color, as
        // `render_with` would compute it, must equal an INDEPENDENTLY
        // constructed `ColorScale::sequential(0.0, 100.0)` (not the
        // figure's own cached instance) evaluated at that same value.
        let area = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 200.0));
        let layout = layout_heatmap(&x_labels, &y_labels, &values, &area);
        let cell = layout.cells.iter().find(|c| c.row == 0 && c.col == 1).expect("cell exists");
        let cell_value = cell.value.expect("cell has a real value");
        assert!((cell_value - 50.0).abs() < 1e-9);

        let independent_scale = ColorScale::sequential(0.0, 100.0);
        assert_eq!(figure.color_scale().color_at(cell_value), independent_scale.color_at(50.0));
    }

    #[test]
    fn missing_or_non_finite_values_still_tile_but_carry_no_value() {
        let x_labels = labels(2, "x");
        let y_labels = labels(2, "y");
        let values = vec![vec![1.0, f64::NAN], vec![2.0]]; // row 1 col 1 missing entirely
        let area = PlotArea::new(Rect::new(0.0, 0.0, 200.0, 200.0));
        let layout = layout_heatmap(&x_labels, &y_labels, &values, &area);
        assert_eq!(layout.cells.len(), 4, "every grid position gets a cell rect regardless of missing/NaN data");
        let nan_cell = layout.cells.iter().find(|c| c.row == 0 && c.col == 1).expect("cell exists");
        assert!(nan_cell.value.is_none());
        let missing_cell = layout.cells.iter().find(|c| c.row == 1 && c.col == 1).expect("cell exists");
        assert!(missing_cell.value.is_none());
        assert!(nan_cell.rect.width > 0.0 && nan_cell.rect.height > 0.0, "a missing-value cell still tiles at full size");
    }

    #[test]
    fn hit_test_cell_resolves_a_point_inside_its_own_rect() {
        let x_labels = labels(3, "x");
        let y_labels = labels(2, "y");
        let values = vec![vec![1.0; 3]; 2];
        let area = PlotArea::new(Rect::new(0.0, 0.0, 300.0, 200.0));
        let layout = layout_heatmap(&x_labels, &y_labels, &values, &area);
        let target = layout.cells.iter().find(|c| c.row == 1 && c.col == 2).expect("cell exists");
        let cx = target.rect.center_x();
        let cy = target.rect.center_y();
        let idx = hit_test_cell(&layout, cx, cy).expect("must hit a cell");
        assert_eq!(layout.cells[idx].row, 1);
        assert_eq!(layout.cells[idx].col, 2);
        assert_eq!(hit_test_cell(&layout, -1000.0, -1000.0), None);
    }

    #[test]
    fn diverging_mode_uses_the_given_mid_as_the_scales_own_midpoint() {
        let x_labels = labels(2, "x");
        let y_labels = labels(1, "y");
        let values = vec![vec![-5.0, 15.0]];
        let figure = HeatmapFigure::new(x_labels, y_labels, values).with_diverging(0.0);
        let scale = figure.color_scale();
        let (mn, mx) = scale.domain();
        assert!((mn - (-5.0)).abs() < 1e-9 && (mx - 15.0).abs() < 1e-9, "domain must be the DATA extent, not a mid-centered rescale");
        // The diverging default mid anchor color must land exactly at 0.0
        // even though 0.0 is not the arithmetic midpoint of [-5, 15].
        let default_diverging = ColorScale::diverging(-5.0, 0.0, 15.0);
        assert_eq!(scale.color_at(0.0), default_diverging.color_at(0.0));
    }

    #[test]
    fn empty_labels_render_without_panicking() {
        let figure = HeatmapFigure::new(Vec::new(), Vec::new(), Vec::new());
        let theme = FigureTheme::dark();
        let spec = uzor_export::ExportSpec { width_px: 200, height_px: 200, dpr: 1.0, background: None };
        let result = uzor_export::render_to_png(&spec, |ctx| {
            figure.render(ctx, Rect::new(0.0, 0.0, 200.0, 200.0), &theme);
        });
        assert!(result.is_ok());
    }
}
