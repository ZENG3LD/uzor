//! Small text-label helpers over `TextRenderer` — used for bar/point value
//! labels and anywhere else a figure wants one already-formatted string
//! drawn at a screen position. Formatting the string itself (precision,
//! thousands separators) is the caller's job — see
//! [`crate::scale::linear::format_value`].

use uzor::render::{RenderContext, TextAlign, TextBaseline};

/// Draw `text` centered on `(x, y)` — horizontal + vertical centering.
pub fn draw_label_centered(ctx: &mut dyn RenderContext, text: &str, x: f64, y: f64, color: &str, font: &str) {
    ctx.set_font(font);
    ctx.set_fill_color(color);
    ctx.set_text_align(TextAlign::Center);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(text, x, y);
}

/// Draw `text` right-aligned at `(x, y)`, vertically centered — the usual
/// placement for a left-side y-axis tick label.
pub fn draw_label_right_aligned(ctx: &mut dyn RenderContext, text: &str, x: f64, y: f64, color: &str, font: &str) {
    ctx.set_font(font);
    ctx.set_fill_color(color);
    ctx.set_text_align(TextAlign::Right);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(text, x, y);
}
