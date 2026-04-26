//! Toast rendering — flat fill, alpha-fade in last 20% of lifetime.
//!
//! Lifecycle math is mlc-style: fade-in over `fade_duration_ms`, hold,
//! then linear fade-out over the last 20% of `duration_ms`. Caller passes
//! current `now_ms` and the toast's `created_at_ms`.

use crate::render::{RenderContext, TextAlign, TextBaseline};
use crate::types::Rect;

use super::settings::ToastSettings;
use super::types::ToastType;

pub struct ToastView<'a> {
    pub toast: &'a ToastType,
    pub created_at_ms: u64,
    pub now_ms: u64,
}

/// Compute current alpha based on lifecycle.
fn alpha_for(toast: &ToastType, created_at_ms: u64, now_ms: u64, fade_ms: u32) -> f64 {
    let elapsed = now_ms.saturating_sub(created_at_ms) as i64;
    let total   = toast.duration_ms as i64;
    let fade    = fade_ms as i64;

    if elapsed < fade {
        // Fade in
        (elapsed as f64 / fade as f64).clamp(0.0, 1.0)
    } else if elapsed > total - fade {
        // Fade out
        let remaining = (total - elapsed).max(0) as f64;
        (remaining / fade as f64).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

pub fn draw_toast(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    view: &ToastView<'_>,
    settings: &ToastSettings,
) {
    let style = settings.style.as_ref();
    let theme = settings.theme.as_ref();

    let alpha = alpha_for(view.toast, view.created_at_ms, view.now_ms, style.fade_duration_ms());
    if alpha <= 0.0 {
        return;
    }

    // Background
    ctx.set_fill_color_alpha(theme.bg_for(view.toast.severity), alpha);
    ctx.fill_rounded_rect(rect.x, rect.y, rect.width, rect.height, style.radius());

    // Message text — left-aligned after the icon area.
    let pad = style.padding();
    let text_x = rect.x + pad + style.icon_size() + pad;
    ctx.set_font(&format!("{}px sans-serif", style.font_size()));
    ctx.set_fill_color_alpha(theme.text(), alpha);
    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);
    ctx.fill_text(&view.toast.message, text_x, rect.y + rect.height / 2.0);
}
