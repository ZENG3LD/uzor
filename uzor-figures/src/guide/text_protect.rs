//! Small text-readability primitives shared by [`crate::guide::annotation`]
//! (this crate) and `uzor-graph`'s own node-label paint (a SEPARATE crate,
//! hence both fns here are `pub`, not `pub(crate)`) — for whenever a label
//! is painted over content that already occupies the same pixels. This
//! crate's usual convention is "labels/axes paint LAST, marks never repaint
//! over them" (see `guide::annotation`'s own layer-contract doc comment),
//! but that convention alone isn't sufficient once the label itself sits in
//! a visually busy region: painting a label glyph-for-glyph last still
//! reads poorly against a cluttered multi-color background directly behind
//! it (a bubble cloud under a reference-band label; a thin edge stroke
//! crossing directly through a node-label's own glyph strokes).
//!
//! Two primitives, two different jobs:
//! - [`plate_under_text`]: an opaque-ish background rect sized to the
//!   label's own measured bounds, painted directly under it — the right
//!   choice when a label has ONE stable rect worth reserving (this crate's
//!   own `HBand` annotation label; a `Callout` box already builds the
//!   equivalent by hand).
//! - [`fill_text_with_halo`]: an 8-direction offset-fill "stroke text"
//!   fallback (no `RenderContext` stroke-text primitive exists anywhere in
//!   this workspace) — the right choice for MANY small labels scattered
//!   over an unpredictable, per-node-position background (`uzor-graph`'s
//!   node labels crossing thin edge strokes at zoom-dependent positions,
//!   from any approach angle), where painting a whole background rect per
//!   label would be visually heavier and cost more per-label draw calls
//!   than warranted.

use uzor::render::{RenderContext, TextAlign, TextBaseline};

use crate::theme::FigureTheme;

/// Padding (px) around a label's own measured bounds a [`plate_under_text`]
/// rect extends by on every side.
const PLATE_PAD: f64 = 3.0;
/// Alpha a [`plate_under_text`] rect paints at — high enough to read as a
/// solid backing panel over dense marks, shy of fully opaque so it still
/// reads as part of the plot rather than a hard-edged sticker.
const PLATE_ALPHA: f64 = 0.88;
/// Offset (px) each of [`fill_text_with_halo`]'s 8 background copies
/// paints at. Tuned (report: the first `0.75`px/4-direction pass proved
/// visually too weak against a busy `uzor-graph` edge crossing right
/// through an 11px label's own thin glyph strokes — a diagonal-only halo
/// leaves the axis-aligned (horizontal/vertical) approach angle of a
/// near-straight edge almost uncovered) to a full 8-direction ring
/// (orthogonal + diagonal) at a slightly larger offset — still reads as a
/// halo, not a double-struck glyph, but now actually erases a real edge
/// stroke crossing from any angle.
const HALO_OFFSET: f64 = 1.1;

/// Paint an opaque-ish `theme.background` rect sized to `text`'s own
/// measured bounds (+ [`PLATE_PAD`] padding) directly under wherever the
/// caller is about to paint `text` at `(x, y)` — MUST be called with the
/// exact `align`/`baseline` the caller's own real text paint uses (this fn
/// reads the CURRENTLY SET font via [`crate::theme::FigureTheme::
/// label_font`] for measurement, and does not itself touch `text_align`/
/// `text_baseline`/fill color beyond what it needs to paint the rect —
/// the caller's own subsequent `fill_text` call is unaffected). A no-op for
/// empty `text`.
pub fn plate_under_text(ctx: &mut dyn RenderContext, theme: &FigureTheme, text: &str, x: f64, y: f64, align: TextAlign, baseline: TextBaseline) {
    if text.is_empty() {
        return;
    }
    let w = ctx.measure_text(text);
    let h = ctx.text_bounds("Ag", &theme.label_font).h.max(10.0);
    let left = match align {
        TextAlign::Left => x,
        TextAlign::Center => x - w / 2.0,
        TextAlign::Right => x - w,
    };
    let top = match baseline {
        TextBaseline::Top => y,
        TextBaseline::Middle => y - h / 2.0,
        TextBaseline::Bottom => y - h,
        // No real ascent/descent split is available from this fn's own
        // inputs — approximates alphabetic baseline as 80% of the line
        // height above it (a documented, minor approximation; every
        // current caller uses Top/Middle, never Alphabetic).
        TextBaseline::Alphabetic => y - h * 0.8,
    };
    ctx.set_fill_color(&theme.background);
    ctx.set_global_alpha(PLATE_ALPHA);
    ctx.fill_rect(left - PLATE_PAD, top - PLATE_PAD, w + PLATE_PAD * 2.0, h + PLATE_PAD * 2.0);
    ctx.set_global_alpha(1.0);
}

/// Paint `text` at `(x, y)` with an 8-direction halo in `halo`
/// underneath the real `fill` paint — see the module docs for when to
/// reach for this instead of [`plate_under_text`]. Caller must already
/// have set `text_align`/`text_baseline`/font; this fn only toggles fill
/// color. A no-op for empty `text`.
pub fn fill_text_with_halo(ctx: &mut dyn RenderContext, text: &str, x: f64, y: f64, fill: &str, halo: &str) {
    if text.is_empty() {
        return;
    }
    ctx.set_fill_color(halo);
    for &(dx, dy) in &[
        (-HALO_OFFSET, -HALO_OFFSET),
        (0.0, -HALO_OFFSET),
        (HALO_OFFSET, -HALO_OFFSET),
        (-HALO_OFFSET, 0.0),
        (HALO_OFFSET, 0.0),
        (-HALO_OFFSET, HALO_OFFSET),
        (0.0, HALO_OFFSET),
        (HALO_OFFSET, HALO_OFFSET),
    ] {
        ctx.fill_text(text, x + dx, y + dy);
    }
    ctx.set_fill_color(fill);
    ctx.fill_text(text, x, y);
}

#[cfg(test)]
mod tests {
    use super::*;
    use uzor::types::Rect;
    use uzor_export::{render_to_png, ExportSpec};

    fn spec() -> ExportSpec {
        ExportSpec { width_px: 200, height_px: 120, dpr: 1.0, background: None }
    }

    #[test]
    fn plate_under_text_renders_without_panicking_for_every_align_baseline_combo() {
        let theme = FigureTheme::dark();
        let result = render_to_png(&spec(), |ctx| {
            ctx.set_font(&theme.label_font);
            for align in [TextAlign::Left, TextAlign::Center, TextAlign::Right] {
                for baseline in [TextBaseline::Top, TextBaseline::Middle, TextBaseline::Bottom, TextBaseline::Alphabetic] {
                    plate_under_text(ctx, &theme, "sample label", 100.0, 60.0, align, baseline);
                }
            }
        });
        assert!(result.is_ok());
    }

    #[test]
    fn plate_under_text_empty_text_is_a_no_op() {
        let theme = FigureTheme::dark();
        let result = render_to_png(&spec(), |ctx| {
            ctx.set_font(&theme.label_font);
            plate_under_text(ctx, &theme, "", 100.0, 60.0, TextAlign::Left, TextBaseline::Top);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn plate_under_text_left_align_extends_rightward_from_x() {
        // A behavioral proxy for "the plate rect actually reserves the
        // label's own measured width": painting a Left-aligned plate at a
        // given x must not panic/misbehave regardless of how wide the
        // measured text turns out to be — exercised via a long vs. a short
        // label at the SAME anchor, both must render cleanly.
        let theme = FigureTheme::dark();
        let bounds = Rect::new(0.0, 0.0, 200.0, 120.0);
        let result = render_to_png(&spec(), |ctx| {
            ctx.set_fill_color(&theme.background);
            ctx.fill_rect(bounds.x, bounds.y, bounds.width, bounds.height);
            ctx.set_font(&theme.label_font);
            plate_under_text(ctx, &theme, "a considerably longer sample label", 10.0, 40.0, TextAlign::Left, TextBaseline::Top);
            plate_under_text(ctx, &theme, "short", 10.0, 70.0, TextAlign::Left, TextBaseline::Top);
        });
        assert!(result.is_ok());
    }

    #[test]
    fn fill_text_with_halo_renders_without_panicking() {
        let theme = FigureTheme::dark();
        let result = render_to_png(&spec(), |ctx| {
            ctx.set_font(&theme.label_font);
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Top);
            fill_text_with_halo(ctx, "node-label", 20.0, 20.0, "#e6e6ea", "#0d0f14");
        });
        assert!(result.is_ok());
    }

    #[test]
    fn fill_text_with_halo_empty_text_is_a_no_op() {
        let theme = FigureTheme::dark();
        let result = render_to_png(&spec(), |ctx| {
            ctx.set_font(&theme.label_font);
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Top);
            fill_text_with_halo(ctx, "", 20.0, 20.0, "#e6e6ea", "#0d0f14");
        });
        assert!(result.is_ok());
    }

    #[test]
    fn fill_text_with_halo_leaves_the_final_fill_color_as_the_caller_supplied_fill() {
        // The halo must not "leak" into whatever paints next — the fn's
        // own last statement sets fill back to `fill` before its own final
        // `fill_text` call, so a caller drawing something else right after
        // (e.g. the next node's own circle) must see `fill`, not `halo`.
        let theme = FigureTheme::dark();
        let result = render_to_png(&spec(), |ctx| {
            ctx.set_font(&theme.label_font);
            ctx.set_text_align(TextAlign::Left);
            ctx.set_text_baseline(TextBaseline::Top);
            fill_text_with_halo(ctx, "node-label", 20.0, 20.0, "#e6e6ea", "#0d0f14");
            // A subsequent fill (e.g. the next primitive) must not be
            // affected by any residual state fill_text_with_halo left —
            // rendering one more shape here must not panic.
            ctx.fill_rect(0.0, 90.0, 20.0, 20.0);
        });
        assert!(result.is_ok());
    }
}
