//! DropdownTrigger render entry point — dispatches over `DropdownTriggerRenderKind`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::{Rect, WidgetState};
use crate::ui::widgets::atomic::button::state::SplitButtonHoverZone;

use super::settings::DropdownTriggerSettings;
use super::style::{DropdownFieldStyle, SplitDropdownStyle};
use super::theme::DropdownTriggerTheme;
use super::types::{DropdownFieldView, DropdownTriggerRenderKind, SplitDropdownView};

/// Render a split dropdown trigger, dispatching on `kind`.
///
/// # Arguments
/// - `ctx`      — render context.
/// - `rect`     — bounding rect.
/// - `state`    — interaction state from the coordinator.
/// - `view`     — per-frame data for Split kind.
/// - `settings` — visual configuration.
/// - `kind`     — which render variant to use.
///
/// For `Split` kind: returns `(text_rect, chevron_rect)` — the two hit-test rects.
/// For `Field` kind: returns `(rect, rect)` — both zones are the same full rect.
/// For `Custom` kind: returns `(rect, rect)` — caller manages zones.
pub fn draw_dropdown_trigger(
    ctx:      &mut dyn RenderContext,
    rect:     Rect,
    state:    WidgetState,
    settings: &DropdownTriggerSettings,
    kind:     &DropdownTriggerRenderKind<'_>,
) -> (Rect, Rect) {
    match kind {
        DropdownTriggerRenderKind::Split => {
            // Split kind requires view data — return identity rects as placeholder.
            // Callers should use draw_split_dropdown directly.
            let _ = state;
            (rect, rect)
        }
        DropdownTriggerRenderKind::Field => {
            let _ = state;
            (rect, rect)
        }
        DropdownTriggerRenderKind::Custom(f) => {
            f(ctx, rect, state, settings);
            (rect, rect)
        }
    }
}

/// Render a split dropdown trigger (section 32).
///
/// # Visual
/// ```text
/// ┌──────────────────────┬────┐
/// │ current_label        │ ▼  │
/// └──────────────────────┴────┘
///  ← text_width ────────→← chevron_width →
/// ```
///
/// # Returns
/// `(text_rect, chevron_rect)` — the two independent hit-test rects.
/// Caller registers each as a separate atomic widget.
pub fn draw_split_dropdown(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &SplitDropdownView<'_>,
    font:  &str,
    style: &dyn SplitDropdownStyle,
    theme: &dyn DropdownTriggerTheme,
) -> (Rect, Rect) {
    let cw = style.chevron_width();
    let text_w = rect.width - cw;

    let text_rect = Rect::new(rect.x,          rect.y, text_w, rect.height);
    let chev_rect = Rect::new(rect.x + text_w, rect.y, cw,     rect.height);

    let r  = style.radius();
    let bw = style.border_width();

    // ── Base background ────────────────────────────────────────────────────────
    ctx.set_fill_color(theme.dropdown_field_bg());
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Hover highlight on the active zone ────────────────────────────────────
    match view.hovered_zone {
        SplitButtonHoverZone::Main => {
            ctx.set_fill_color(theme.dropdown_field_bg_hover());
            ctx.save();
            ctx.clip_rect(rect.x, rect.y, text_w, rect.height);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
            ctx.restore();
        }
        SplitButtonHoverZone::Chevron => {
            ctx.set_fill_color(theme.dropdown_field_bg_hover());
            ctx.save();
            ctx.clip_rect(rect.x + text_w, rect.y, cw, rect.height);
            ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);
            ctx.restore();
        }
        SplitButtonHoverZone::None => {}
    }

    // ── Border ─────────────────────────────────────────────────────────────────
    ctx.set_stroke_color(theme.dropdown_field_border());
    ctx.set_stroke_width(bw);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Label ─────────────────────────────────────────────────────────────────
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_fill_color(theme.dropdown_field_text());
    ctx.fill_text(view.current_label, rect.x + style.text_padding_x(), rect.center_y());

    // ── Vertical separator ────────────────────────────────────────────────────
    ctx.set_stroke_color(theme.dropdown_field_border());
    ctx.set_stroke_width(bw);
    ctx.set_line_dash(&[]);
    ctx.begin_path();
    ctx.move_to(rect.x + text_w, rect.y);
    ctx.line_to(rect.x + text_w, rect.y + rect.height);
    ctx.stroke();

    // ── Filled triangle chevron ───────────────────────────────────────────────
    let arrow_cx = chev_rect.center_x();
    let arrow_cy = chev_rect.center_y();
    let hb = 3.0_f64;
    let ah = 3.0_f64;
    ctx.set_fill_color(theme.dropdown_chevron_color());
    ctx.begin_path();
    ctx.move_to(arrow_cx - hb, arrow_cy - ah);
    ctx.line_to(arrow_cx + hb, arrow_cy - ah);
    ctx.line_to(arrow_cx,      arrow_cy + ah);
    ctx.close_path();
    ctx.fill();

    (text_rect, chev_rect)
}

/// Render a dropdown field trigger (section 33).
///
/// # Visual
/// ```text
/// ╭─────────────────────────╮
/// │ current_label      ↓   │
/// ╰─────────────────────────╯
/// ```
///
/// # Returns
/// The full `rect` as the single hit-test rect (no split zones).
pub fn draw_dropdown_field(
    ctx:   &mut dyn RenderContext,
    rect:  Rect,
    view:  &DropdownFieldView<'_>,
    font:  &str,
    style: &dyn DropdownFieldStyle,
    theme: &dyn DropdownTriggerTheme,
) -> Rect {
    let r   = style.radius();
    let bw  = style.border_width();
    let cs  = style.chevron_size();
    let cmr = style.chevron_margin_right();

    // ── Background ─────────────────────────────────────────────────────────────
    let bg = if view.hovered || view.open {
        theme.dropdown_field_bg_hover()
    } else {
        theme.dropdown_field_bg()
    };
    ctx.set_fill_color(bg);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Border ─────────────────────────────────────────────────────────────────
    ctx.set_stroke_color(theme.dropdown_field_border());
    ctx.set_stroke_width(bw);
    ctx.set_line_dash(&[]);
    ctx.stroke_rounded_rect(rect.x, rect.y, rect.width, rect.height, r);

    // ── Label ─────────────────────────────────────────────────────────────────
    ctx.set_font(font);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.set_fill_color(theme.dropdown_field_text());
    ctx.fill_text(view.current_label, rect.x + style.text_padding_x(), rect.center_y());

    // ── Chevron (filled triangle, cs×cs bounding box) ────────────────────────
    let arrow_x  = rect.x + rect.width - cs - cmr;
    let arrow_cy = rect.center_y();
    let hb  = cs / 4.0;
    let ah  = cs / 4.0;
    let acx = arrow_x + cs / 2.0;
    ctx.set_fill_color(theme.dropdown_chevron_color());
    ctx.begin_path();
    ctx.move_to(acx - hb, arrow_cy - ah);
    ctx.line_to(acx + hb, arrow_cy - ah);
    ctx.line_to(acx,      arrow_cy + ah);
    ctx.close_path();
    ctx.fill();

    rect
}
