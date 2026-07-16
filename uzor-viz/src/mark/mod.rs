//! Stateless mark draw functions — `(ctx, area, scale(s), data, style) ->
//! ()`. Every function only writes to `ctx`; [`crate::coord::PlotArea`]
//! plus the scale(s) passed in are the only positioning inputs (design
//! laws #1 and #3 — one shared transform, pure draw over borrowed data).
//!
//! `BatchPainter` is used wherever the shape repeats per-datapoint
//! (`draw_line_batch`/`draw_circle_batch`/`stroke_polyline`) — same rule
//! `uzor-graph::render` already follows.

pub mod area;
pub mod line;
pub mod point;
pub mod rect;
pub mod text;

/// Minimal per-mark visual style.
///
/// `color` is a CSS hex string (`"#rrggbb"` / `"#rrggbbaa"`), matching
/// every `set_fill_color`/`set_stroke_color` call across the uzor render
/// stack (same convention as `uzor-graph::render::category_color`).
#[derive(Debug, Clone)]
pub struct MarkStyle {
    pub color: String,
    pub stroke_width: f64,
    pub fill_alpha: f64,
}

impl Default for MarkStyle {
    fn default() -> Self {
        Self { color: "#4d90fe".to_owned(), stroke_width: 1.5, fill_alpha: 1.0 }
    }
}
