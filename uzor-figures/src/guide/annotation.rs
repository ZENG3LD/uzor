//! Annotation guide — reference lines, reference bands, and text callouts
//! drawn OVER a figure's own marks/axes, through the same
//! [`PlotArea`]/[`Scale`] transform every mark/guide in this crate already
//! uses (design law #1). A figure opts in via a small `annotations: Vec<
//! Annotation>` field + a `.with_annotations(..)` builder — the SAME
//! composition idiom every other optional figure capability in this crate
//! already uses (`title`, `fill`, `legend_position`, `downsample_max`,
//! ...), never a config bag and never a change to any figure's own
//! `render`/`render_with` signature (see e.g.
//! [`crate::figure::CurveFigure::with_downsample`]).
//!
//! [`Annotation::Callout`]'s flip-to-fit box placement is adapted from
//! [`crate::guide::tooltip::draw_tooltip`]'s own edge-flip idiom — a
//! callout is a PERMANENTLY-drawn annotation (no live cursor position),
//! so it additionally draws a leader line from its own anchor point to the
//! label box (a hover tooltip never needs one — the cursor itself already
//! visually marks the anchor) and a small anchor dot; the label itself is
//! single free-text (not `tooltip::draw_tooltip`'s own key/value two-column
//! row list, which reads oddly for one plain sentence).

use uzor::render::{CircleBatch, RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::coord::PlotArea;
use crate::scale::Scale;
use crate::theme::FigureTheme;

const REFERENCE_DASH: [f64; 2] = [5.0, 3.0];
const REFERENCE_LABEL_GAP: f64 = 4.0;
const BAND_ALPHA: f64 = 0.14;

const CALLOUT_PAD: f64 = 6.0;
const CALLOUT_OFFSET: f64 = 10.0;
const CALLOUT_DOT_RADIUS: f64 = 3.0;
const CALLOUT_CORNER_RADIUS: f64 = 3.0;
const CALLOUT_ALPHA: f64 = 0.94;

/// One annotation over a figure's plot area. Domain values (`value`/`low`/
/// `high`/`x`/`y`) are always in the SAME domain space the figure's own
/// scale(s) already use — an annotation never carries its own pixel
/// geometry.
#[derive(Debug, Clone)]
pub enum Annotation {
    /// A horizontal dashed reference line at a fixed Y domain value,
    /// spanning the plot's full width (e.g. a target/threshold line).
    HLine { value: f64, color: Option<String>, label: Option<String> },
    /// A vertical dashed reference line at a fixed X domain value,
    /// spanning the plot's full height (e.g. a notable date/event marker).
    VLine { value: f64, color: Option<String>, label: Option<String> },
    /// A shaded horizontal band over a Y domain interval `[low, high]`
    /// (order-independent), spanning the plot's full width — e.g. a
    /// "normal range" or confidence band.
    HBand { low: f64, high: f64, color: Option<String>, label: Option<String> },
    /// A free-text callout anchored at one `(x, y)` domain point, with a
    /// leader line to a small flip-to-fit label box (see the module docs).
    Callout { x: f64, y: f64, text: String },
}

fn dashed_hline(ctx: &mut dyn RenderContext, area: &PlotArea, yscale: &dyn Scale, theme: &FigureTheme, value: f64, color: &Option<String>, label: &Option<String>) {
    let y = area.y(yscale, value);
    let stroke = color.as_deref().unwrap_or(&theme.label_color);
    ctx.set_stroke_color(stroke);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&REFERENCE_DASH);
    ctx.begin_path();
    ctx.move_to(area.rect.x, y);
    ctx.line_to(area.rect.right(), y);
    ctx.stroke();
    ctx.set_line_dash(&[]);

    if let Some(text) = label {
        ctx.set_font(&theme.label_font);
        ctx.set_fill_color(stroke);
        ctx.set_text_align(TextAlign::Right);
        ctx.set_text_baseline(TextBaseline::Bottom);
        ctx.fill_text(text, area.rect.right() - REFERENCE_LABEL_GAP, y - REFERENCE_LABEL_GAP);
    }
}

fn dashed_vline(ctx: &mut dyn RenderContext, area: &PlotArea, xscale: &dyn Scale, theme: &FigureTheme, value: f64, color: &Option<String>, label: &Option<String>) {
    let x = area.x(xscale, value);
    let stroke = color.as_deref().unwrap_or(&theme.label_color);
    ctx.set_stroke_color(stroke);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&REFERENCE_DASH);
    ctx.begin_path();
    ctx.move_to(x, area.rect.y);
    ctx.line_to(x, area.rect.bottom());
    ctx.stroke();
    ctx.set_line_dash(&[]);

    if let Some(text) = label {
        ctx.set_font(&theme.label_font);
        ctx.set_fill_color(stroke);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(text, x + REFERENCE_LABEL_GAP, area.rect.y + REFERENCE_LABEL_GAP);
    }
}

fn shaded_hband(ctx: &mut dyn RenderContext, area: &PlotArea, yscale: &dyn Scale, theme: &FigureTheme, low: f64, high: f64, color: &Option<String>, label: &Option<String>) {
    let y0 = area.y(yscale, low);
    let y1 = area.y(yscale, high);
    let (top, height) = (y0.min(y1), (y1 - y0).abs());
    let fill = color.as_deref().unwrap_or(&theme.label_color);
    ctx.set_fill_color(fill);
    ctx.set_global_alpha(BAND_ALPHA);
    ctx.fill_rect(area.rect.x, top, area.rect.width, height);
    ctx.set_global_alpha(1.0);

    if let Some(text) = label {
        ctx.set_font(&theme.label_font);
        ctx.set_fill_color(fill);
        ctx.set_text_align(TextAlign::Left);
        ctx.set_text_baseline(TextBaseline::Top);
        ctx.fill_text(text, area.rect.x + REFERENCE_LABEL_GAP, top + REFERENCE_LABEL_GAP);
    }
}

/// Free-text callout at a resolved screen `anchor_px`, staying inside
/// `bounds` — flips horizontally (right-of-anchor by default, left when
/// that would overflow `bounds`) and vertically (above-of-anchor by
/// default, below when that would overflow `bounds`), same flip-to-fit
/// spirit as [`crate::guide::tooltip::draw_tooltip`] (see the module docs
/// for the two ways this diverges from it). No-op for empty `text`.
fn draw_callout(ctx: &mut dyn RenderContext, theme: &FigureTheme, anchor_px: (f64, f64), text: &str, bounds: Rect) {
    if text.is_empty() {
        return;
    }
    let (anchor_x, anchor_y) = anchor_px;

    ctx.draw_circle_batch(&[CircleBatch { cx: anchor_x, cy: anchor_y, r: CALLOUT_DOT_RADIUS }], &theme.label_color);

    ctx.set_font(&theme.label_font);
    let text_w = ctx.measure_text(text);
    let text_h = ctx.text_bounds("Ag", &theme.label_font).h.max(12.0);
    let box_w = CALLOUT_PAD * 2.0 + text_w;
    let box_h = CALLOUT_PAD * 2.0 + text_h;

    let mut box_x = anchor_x + CALLOUT_OFFSET;
    if box_x + box_w > bounds.right() {
        box_x = (anchor_x - CALLOUT_OFFSET - box_w).max(bounds.x);
    }
    let mut box_y = anchor_y - box_h - CALLOUT_OFFSET;
    if box_y < bounds.y {
        box_y = (anchor_y + CALLOUT_OFFSET).min((bounds.bottom() - box_h).max(bounds.y));
    }
    if box_y + box_h > bounds.bottom() {
        box_y = (bounds.bottom() - box_h).max(bounds.y);
    }

    ctx.set_stroke_color(&theme.label_color);
    ctx.set_stroke_width(1.0);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(anchor_x, anchor_y);
    ctx.line_to((box_x + box_w / 2.0).clamp(box_x, box_x + box_w), (box_y + box_h / 2.0).clamp(box_y, box_y + box_h));
    ctx.stroke();

    ctx.set_fill_color(&theme.background);
    ctx.set_global_alpha(CALLOUT_ALPHA);
    ctx.fill_rounded_rect(box_x, box_y, box_w, box_h, CALLOUT_CORNER_RADIUS);
    ctx.set_global_alpha(1.0);
    ctx.set_stroke_color(&theme.axis_color);
    ctx.stroke_rounded_rect(box_x, box_y, box_w, box_h, CALLOUT_CORNER_RADIUS);

    ctx.set_font(&theme.label_font);
    ctx.set_fill_color(&theme.label_color);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(text, box_x + CALLOUT_PAD, box_y + box_h / 2.0);
}

/// Draw every `annotations` entry over `area` — `xscale`/`yscale` are
/// whatever the calling figure already resolved for this render pass
/// (design law #1: no annotation-private scale math). Order: `annotations`
/// draws in caller order, so a later entry paints over an earlier one.
pub fn draw_annotations(ctx: &mut dyn RenderContext, area: &PlotArea, xscale: &dyn Scale, yscale: &dyn Scale, theme: &FigureTheme, annotations: &[Annotation]) {
    for annotation in annotations {
        match annotation {
            Annotation::HLine { value, color, label } => dashed_hline(ctx, area, yscale, theme, *value, color, label),
            Annotation::VLine { value, color, label } => dashed_vline(ctx, area, xscale, theme, *value, color, label),
            Annotation::HBand { low, high, color, label } => shaded_hband(ctx, area, yscale, theme, *low, *high, color, label),
            Annotation::Callout { x, y, text } => {
                let anchor_px = (area.x(xscale, *x), area.y(yscale, *y));
                draw_callout(ctx, theme, anchor_px, text, area.rect);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::LinearScale;
    use uzor_export::{render_to_png, ExportSpec};

    fn area() -> PlotArea {
        PlotArea::new(Rect::new(20.0, 20.0, 200.0, 150.0))
    }

    #[test]
    fn draw_annotations_renders_every_variant_without_panicking() {
        let theme = FigureTheme::dark();
        let xscale = LinearScale::new(0.0, 100.0);
        let yscale = LinearScale::new(0.0, 50.0);
        let annotations = vec![
            Annotation::HLine { value: 25.0, color: None, label: Some("target".to_owned()) },
            Annotation::VLine { value: 60.0, color: Some("#ff0000".to_owned()), label: None },
            Annotation::HBand { low: 10.0, high: 20.0, color: None, label: Some("range".to_owned()) },
            Annotation::Callout { x: 80.0, y: 40.0, text: "notable point".to_owned() },
        ];
        let spec = ExportSpec { width_px: 240, height_px: 190, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_annotations(ctx, &area(), &xscale, &yscale, &theme, &annotations);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn draw_annotations_empty_slice_is_a_no_op() {
        let theme = FigureTheme::dark();
        let xscale = LinearScale::new(0.0, 100.0);
        let yscale = LinearScale::new(0.0, 50.0);
        let spec = ExportSpec { width_px: 240, height_px: 190, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_annotations(ctx, &area(), &xscale, &yscale, &theme, &[]);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn callout_with_empty_text_draws_nothing_but_still_does_not_panic() {
        let theme = FigureTheme::dark();
        let spec = ExportSpec { width_px: 240, height_px: 190, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_callout(ctx, &theme, (100.0, 100.0), "", area().rect);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn callout_near_the_right_edge_flips_the_box_to_the_left() {
        // A callout anchored right at the plot's own right edge cannot fit
        // its box on the default (right-of-anchor) side — this is a
        // behavioral smoke test (render succeeds, no panic/overflow-driven
        // NaN); the flip MATH itself mirrors `tooltip::draw_tooltip`'s own
        // already-tested `box_x + box_w > bounds.right()` branch.
        let theme = FigureTheme::dark();
        let bounds = area().rect;
        let anchor_px = (bounds.right() - 2.0, bounds.y + 5.0);
        let spec = ExportSpec { width_px: 240, height_px: 190, dpr: 1.0, background: None };
        let result = render_to_png(&spec, |ctx| {
            draw_callout(ctx, &theme, anchor_px, "a fairly long callout label", bounds);
        });
        assert!(result.is_ok());
    }
}
