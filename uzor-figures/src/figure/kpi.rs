//! `KpiFigure` — a single dashboard "number tile": a big headline value, an
//! optional delta vs. a previous value (colored via
//! [`crate::theme::FigureTheme::positive`]/`negative`, an up/down
//! triangle glyph — the FT/business-dashboard staple), and an optional
//! trailing sparkline (reusing [`crate::mark::line::draw_polyline`]/
//! [`crate::mark::area::draw_area`], no axes/grid — the "just the shape"
//! sparkline convention). Sized to sit in a dashboard grid — a caller
//! composes a "KPI row" by placing several `KpiFigure`s at their own
//! sub-rects (this crate's own figures are always single-widget; see
//! `nemo/uzor-typeset`'s showcase for a real multi-tile row via a table of
//! `Block::Figure` cells).

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::figure::FigureOverlay;
use crate::mark::area::draw_area;
use crate::mark::line::draw_polyline;
use crate::mark::MarkStyle;
use crate::scale::{LinearScale, NumberFormat};
use crate::theme::FigureTheme;

const PAD: f64 = 12.0;
const LABEL_TO_VALUE_GAP: f64 = 4.0;
const VALUE_TO_DELTA_GAP: f64 = 4.0;
const DELTA_TO_SPARK_GAP: f64 = 8.0;
/// Fraction of the tile's own inner height a sparkline strip reserves at
/// the bottom, when present.
const SPARKLINE_HEIGHT_FRACTION: f64 = 0.32;
const SPARKLINE_MIN_HEIGHT: f64 = 16.0;
const SPARKLINE_FILL_ALPHA: f64 = 0.18;
const DELTA_TRIANGLE_SIZE: f64 = 6.0;
const DELTA_TRIANGLE_GAP: f64 = 4.0;
/// Stroke alpha of the subtle accent border a hover position inside the
/// tile draws around its own edge.
const HOVER_BORDER_ALPHA: f64 = 0.5;

/// A single dashboard number tile — see the module docs.
pub struct KpiFigure {
    pub label: String,
    pub value: f64,
    pub previous_value: Option<f64>,
    pub sparkline: Vec<f64>,
    format: NumberFormat,
}

impl KpiFigure {
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self { label: label.into(), value, previous_value: None, sparkline: Vec::new(), format: NumberFormat::default() }
    }

    pub fn with_previous_value(mut self, previous: f64) -> Self {
        self.previous_value = Some(previous);
        self
    }

    /// Chronological values (oldest first) for the trailing sparkline —
    /// fewer than 2 points draws no sparkline (same "nothing to shape"
    /// convention [`crate::mark::line::draw_polyline`]'s own docs state).
    pub fn with_sparkline(mut self, values: Vec<f64>) -> Self {
        self.sparkline = values;
        self
    }

    pub fn with_format(mut self, format: NumberFormat) -> Self {
        self.format = format;
        self
    }

    /// `(value - previous) / |previous| * 100` — `None` when there's no
    /// [`KpiFigure::previous_value`], or it's exactly `0.0` (a percent
    /// change against a zero baseline is undefined, not "infinite").
    pub fn delta_pct(&self) -> Option<f64> {
        let previous = self.previous_value?;
        if previous == 0.0 {
            return None;
        }
        Some((self.value - previous) / previous.abs() * 100.0)
    }

    fn has_sparkline(&self) -> bool {
        self.sparkline.len() >= 2
    }

    /// Derive a scaled font string from `theme.label_font` (e.g.
    /// `"11px sans-serif"` -> `"bold 28px sans-serif"`) — this crate's
    /// [`FigureTheme`] carries exactly one font size (its own small guide/
    /// label text); a KPI headline number needs a visibly larger weight
    /// class no existing theme field carries. Expects the SAME `"<N>px
    /// <family...>"` shape every [`FigureTheme::label_font`] in this crate
    /// already uses — when the first token isn't a valid `px` size (a
    /// malformed/unexpected theme), the WHOLE family falls back to a
    /// literal `"sans-serif"` rather than misreading an arbitrary token as
    /// the family — never panics either way.
    ///
    /// [`FigureTheme::label_font`]: crate::theme::FigureTheme::label_font
    fn scaled_font(label_font: &str, size_px: f64, bold: bool) -> String {
        let mut tokens = label_font.split_whitespace();
        let first_is_size = tokens.next().is_some_and(|t| t.strip_suffix("px").is_some_and(|n| n.parse::<f64>().is_ok()));
        let family = if first_is_size {
            let rest: Vec<&str> = tokens.collect();
            if rest.is_empty() { "sans-serif".to_owned() } else { rest.join(" ") }
        } else {
            "sans-serif".to_owned()
        };
        if bold {
            format!("bold {size_px}px {family}")
        } else {
            format!("{size_px}px {family}")
        }
    }

    /// Render into `rect` of `ctx` using `theme`, with no overlay —
    /// equivalent to `render_with(ctx, rect, theme, &FigureOverlay::default())`.
    pub fn render(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme) {
        self.render_with(ctx, rect, theme, &FigureOverlay::default());
    }

    /// Render into `rect` of `ctx` using `theme`. `overlay.hover_px`
    /// inside `rect` draws a subtle accent border around the whole tile
    /// (every other field of `overlay` is unconsumed — a KPI tile has no
    /// finer-grained hit-test target than "the whole tile").
    pub fn render_with(&self, ctx: &mut dyn RenderContext, rect: Rect, theme: &FigureTheme, overlay: &FigureOverlay<'_>) {
        ctx.set_fill_color(&theme.background);
        ctx.fill_rect(rect.x, rect.y, rect.width, rect.height);

        let inner = Rect::new(rect.x + PAD, rect.y + PAD, (rect.width - PAD * 2.0).max(0.0), (rect.height - PAD * 2.0).max(0.0));

        let spark_h = if self.has_sparkline() { (inner.height * SPARKLINE_HEIGHT_FRACTION).max(SPARKLINE_MIN_HEIGHT) } else { 0.0 };
        let text_h = (inner.height - spark_h - if self.has_sparkline() { DELTA_TO_SPARK_GAP } else { 0.0 }).max(0.0);

        let label_font = theme.label_font.clone();
        let value_font_size = (text_h * 0.42).clamp(16.0, 40.0);
        let value_font = Self::scaled_font(&label_font, value_font_size, true);
        let delta_font_size = (text_h * 0.16).clamp(11.0, 16.0);
        let delta_font = Self::scaled_font(&label_font, delta_font_size, false);

        let mut cursor_y = inner.y;

        // Label.
        ctx.set_font(&label_font);
        ctx.set_fill_color(&theme.label_color);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(&self.label, inner.x, cursor_y);
        let label_h = ctx.text_bounds(&self.label, &label_font).h.max(12.0);
        cursor_y += label_h + LABEL_TO_VALUE_GAP;

        // Big value.
        let step = self.sparkline.iter().chain(std::iter::once(&self.value)).copied().fold(1.0_f64, |acc, v| acc.max(v.abs())).max(1.0) / 100.0;
        let value_text = self.format.format(self.value, step);
        ctx.set_font(&value_font);
        ctx.set_fill_color(&theme.label_color);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(&value_text, inner.x, cursor_y);
        let value_h = ctx.text_bounds(&value_text, &value_font).h.max(value_font_size);
        cursor_y += value_h + VALUE_TO_DELTA_GAP;

        // Delta (up/down triangle + percent), if there's a previous value.
        // A genuinely FLAT delta (`== 0.0`) draws no directional triangle
        // at all (an up-glyph colored neutral would still visually imply
        // "up") and no leading `+` sign — text alone, `theme.label_color`.
        if let Some(delta) = self.delta_pct() {
            let (color, text_x) = if delta > 0.0 {
                let tri_cy = cursor_y + delta_font_size * 0.5;
                draw_delta_triangle(ctx, inner.x + DELTA_TRIANGLE_SIZE, tri_cy, DELTA_TRIANGLE_SIZE, true, &theme.positive);
                (&theme.positive, inner.x + DELTA_TRIANGLE_SIZE * 2.0 + DELTA_TRIANGLE_GAP)
            } else if delta < 0.0 {
                let tri_cy = cursor_y + delta_font_size * 0.5;
                draw_delta_triangle(ctx, inner.x + DELTA_TRIANGLE_SIZE, tri_cy, DELTA_TRIANGLE_SIZE, false, &theme.negative);
                (&theme.negative, inner.x + DELTA_TRIANGLE_SIZE * 2.0 + DELTA_TRIANGLE_GAP)
            } else {
                (&theme.label_color, inner.x)
            };

            let delta_text = if delta > 0.0 { format!("+{delta:.1}%") } else { format!("{delta:.1}%") };
            ctx.set_font(&delta_font);
            ctx.set_fill_color(color);
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Top);
            ctx.fill_text(&delta_text, text_x, cursor_y);
        }

        // Sparkline.
        if self.has_sparkline() {
            let spark_rect = Rect::new(inner.x, inner.bottom() - spark_h, inner.width, spark_h);
            let area = PlotArea::new(spark_rect);
            let x_scale = LinearScale::new(0.0, (self.sparkline.len() - 1) as f64);
            let (y_min, y_max) = self.sparkline.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(mn, mx), &v| (mn.min(v), mx.max(v)));
            let y_scale = LinearScale::new(y_min, y_max);
            let points: Vec<(f64, f64)> = self.sparkline.iter().enumerate().map(|(i, &v)| (i as f64, v)).collect();

            let trending_up = self.sparkline.last().copied().unwrap_or(0.0) >= self.sparkline.first().copied().unwrap_or(0.0);
            let color = if trending_up { theme.positive.clone() } else { theme.negative.clone() };
            let fill_style = MarkStyle { color: color.clone(), fill_alpha: SPARKLINE_FILL_ALPHA, ..Default::default() };
            let line_style = MarkStyle { color, stroke_width: 1.5, ..Default::default() };
            draw_area(ctx, &area, &x_scale, &y_scale, &points, &fill_style);
            draw_polyline(ctx, &area, &x_scale, &y_scale, &points, &line_style);
        }

        if let Some((hx, hy)) = overlay.hover_px {
            if hx >= rect.x && hx <= rect.right() && hy >= rect.y && hy <= rect.bottom() {
                ctx.set_stroke_color(&theme.palette[0]);
                ctx.set_stroke_width(1.5);
                ctx.set_global_alpha(HOVER_BORDER_ALPHA);
                ctx.stroke_rect(rect.x + 0.75, rect.y + 0.75, (rect.width - 1.5).max(0.0), (rect.height - 1.5).max(0.0));
                ctx.set_global_alpha(1.0);
            }
        }
    }
}

/// A small filled up/down triangle at `(cx, cy)` of half-width/height
/// `size` — a hand-built path (3 points, fill), same "no font-glyph
/// dependency for a tiny geometric marker" reasoning
/// [`crate::figure::pie::append_arc`]'s own bezier-arc builder documents
/// (a Unicode `▲`/`▼` glyph would depend on the embedded font actually
/// shipping that codepoint, unverified across every backend this crate
/// renders through).
fn draw_delta_triangle(ctx: &mut dyn RenderContext, cx: f64, cy: f64, size: f64, up: bool, color: &str) {
    ctx.set_fill_color(color);
    ctx.begin_path();
    if up {
        ctx.move_to(cx, cy - size);
        ctx.line_to(cx + size, cy + size * 0.6);
        ctx.line_to(cx - size, cy + size * 0.6);
    } else {
        ctx.move_to(cx, cy + size);
        ctx.line_to(cx + size, cy - size * 0.6);
        ctx.line_to(cx - size, cy - size * 0.6);
    }
    ctx.close_path();
    ctx.fill();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_pct_computes_the_signed_percent_change() {
        let up = KpiFigure::new("Revenue", 120.0).with_previous_value(100.0);
        assert!((up.delta_pct().unwrap() - 20.0).abs() < 1e-9);
        let down = KpiFigure::new("Churn", 80.0).with_previous_value(100.0);
        assert!((down.delta_pct().unwrap() - (-20.0)).abs() < 1e-9);
    }

    #[test]
    fn delta_pct_is_none_without_a_previous_value() {
        assert!(KpiFigure::new("Revenue", 120.0).delta_pct().is_none());
    }

    #[test]
    fn delta_pct_is_none_against_a_zero_previous_value() {
        assert!(KpiFigure::new("Revenue", 120.0).with_previous_value(0.0).delta_pct().is_none());
    }

    #[test]
    fn delta_pct_handles_a_negative_previous_value_via_the_absolute_denominator() {
        // previous = -50, value = -25 -> a 50% move TOWARD zero, still a
        // well-defined magnitude via `previous.abs()` in the denominator.
        let figure = KpiFigure::new("Net", -25.0).with_previous_value(-50.0);
        assert!((figure.delta_pct().unwrap() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn scaled_font_extracts_the_family_and_applies_bold_and_size() {
        assert_eq!(KpiFigure::scaled_font("11px sans-serif", 28.0, true), "bold 28px sans-serif");
        assert_eq!(KpiFigure::scaled_font("11px sans-serif", 13.0, false), "13px sans-serif");
    }

    #[test]
    fn scaled_font_falls_back_to_sans_serif_for_a_malformed_theme_font() {
        assert_eq!(KpiFigure::scaled_font("garbage", 20.0, true), "bold 20px sans-serif");
    }

    #[test]
    fn render_smoke_with_and_without_sparkline_and_previous_value() {
        use uzor_export::{render_to_png, ExportSpec};

        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 220, height_px: 120, dpr: 1.0, background: None };
        let rect = Rect::new(0.0, 0.0, 220.0, 120.0);

        let bare = KpiFigure::new("Active Users", 48213.0);
        let result = render_to_png(&spec, |ctx| bare.render(ctx, rect, &theme));
        assert!(result.is_ok());

        let with_delta = KpiFigure::new("Revenue", 128_400.0).with_previous_value(110_000.0).with_format(NumberFormat::Currency("$"));
        let result = render_to_png(&spec, |ctx| with_delta.render(ctx, rect, &theme));
        assert!(result.is_ok());

        let sparkline: Vec<f64> = (0..20).map(|i| 100.0 + ((i * 7) % 15) as f64).collect();
        let full =
            KpiFigure::new("Signups", 812.0).with_previous_value(790.0).with_sparkline(sparkline).with_format(NumberFormat::Si);
        let overlay = FigureOverlay { hover_px: Some((100.0, 60.0)), brush: None, focus: None };
        let result = render_to_png(&spec, |ctx| full.render_with(ctx, rect, &theme, &overlay));
        assert!(result.is_ok());
    }

    #[test]
    fn a_flat_delta_uses_the_neutral_label_color_not_positive_or_negative() {
        use uzor_export::{render_to_png, ExportSpec};
        let figure = KpiFigure::new("Steady", 100.0).with_previous_value(100.0);
        assert!((figure.delta_pct().unwrap()).abs() < 1e-9);
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 220, height_px: 120, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| figure.render(ctx, Rect::new(0.0, 0.0, 220.0, 120.0), &theme));
        assert!(result.is_ok());
    }
}
