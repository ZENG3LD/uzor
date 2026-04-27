//! Toast rendering — mlc-parity flat-rect path.
//!
//! Render math mirrors mlc `render_toasts` exactly:
//! - No fade-in.
//! - Linear fade-out over the **last 20 %** of lifetime.
//! - Shadow offset 2 px, dark-navy bg, blue accent border (four fill_rect sides),
//!   bold-12px title in accent blue, 11px message in muted white.
//! - Stack: top-right anchor, downward, break on overflow.
//!
//! Per-severity accent colour is the only extension beyond mlc (uzor choice B).

use crate::render::{RenderContext, TextAlign, TextBaseline};

use super::state::ToastEntry;
use super::style::ToastGeometry as G;
use super::theme::{accent_rgb, rgba, MLC_BG, MLC_BG_ALPHA, MLC_BORDER_ALPHA, MLC_SHADOW, MLC_SHADOW_ALPHA, MLC_TEXT, MLC_TEXT_ALPHA, MLC_TITLE_ALPHA};
use super::types::ToastType;

// ─── Alpha ────────────────────────────────────────────────────────────────────

/// Compute fade alpha, matching mlc `render_toasts` exactly.
///
/// - `remaining >= 0.2` → `1.0`
/// - `remaining < 0.2`  → `remaining / 0.2` (linear 1→0)
/// - `remaining <= 0.0` → `0.0` (caller skips)
///
/// This differs from the old uzor path which also had a fade-in phase.
pub fn alpha_for(toast: &ToastType, now_ms: u64) -> f64 {
    let remaining = toast.remaining_fraction(now_ms);
    if remaining < G::FADE_THRESHOLD {
        (remaining / G::FADE_THRESHOLD).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

// ─── Single toast ─────────────────────────────────────────────────────────────

/// Draw one toast at absolute position `(x, y)`.
///
/// Matches mlc's per-iteration body in `render_toasts`:
/// shadow → bg → border (4 sides) → title → message.
pub fn draw_toast_at(
    ctx: &mut dyn RenderContext,
    x: f64,
    y: f64,
    entry: &ToastEntry,
    now_ms: u64,
) {
    let toast = &entry.toast;
    let alpha = alpha_for(toast, now_ms);
    if alpha <= 0.0 {
        return;
    }

    let w = G::TOAST_WIDTH;
    let h = G::TOAST_HEIGHT;
    let bt = G::BORDER_THICKNESS;
    let pad = G::PADDING;
    let so = G::SHADOW_OFFSET;
    let accent = accent_rgb(toast.severity);

    // Shadow
    ctx.set_fill_color(&rgba(MLC_SHADOW, alpha * MLC_SHADOW_ALPHA));
    ctx.fill_rect(x + so, y + so, w, h);

    // Background
    ctx.set_fill_color(&rgba(MLC_BG, alpha * MLC_BG_ALPHA));
    ctx.fill_rect(x, y, w, h);

    // Border — four filled rects (mlc uses no border-radius)
    let border_color = rgba(accent, alpha * MLC_BORDER_ALPHA);
    ctx.set_fill_color(&border_color);
    ctx.fill_rect(x, y, w, bt);                              // top
    ctx.fill_rect(x, y + h - bt, w, bt);                    // bottom
    ctx.fill_rect(x, y + bt, bt, h - bt * 2.0);             // left
    ctx.fill_rect(x + w - bt, y + bt, bt, h - bt * 2.0);   // right

    ctx.set_text_align(TextAlign::Left);
    ctx.set_text_baseline(TextBaseline::Middle);

    // Title (if present) — bold 12px, accent colour
    let title_str: Option<&str> = toast.title.as_deref();
    if let Some(title) = title_str {
        ctx.set_font("bold 12px sans-serif");
        ctx.set_fill_color(&rgba(accent, alpha * MLC_TITLE_ALPHA));
        ctx.fill_text(title, x + pad, y + G::TITLE_Y_OFFSET);
    }

    // Message — 11px, muted white
    ctx.set_font("11px sans-serif");
    ctx.set_fill_color(&rgba(MLC_TEXT, alpha * MLC_TEXT_ALPHA));

    // If no title, vertically centre the message in the card.
    let msg_y = if title_str.is_some() {
        y + G::MESSAGE_Y_OFFSET
    } else {
        y + h / 2.0
    };
    ctx.fill_text(&toast.message, x + pad, msg_y);
}

// ─── Stack renderer ───────────────────────────────────────────────────────────

/// Draw the full toast stack top-right anchored, matching mlc layout.
///
/// - `window_width` / `window_height` — canvas dimensions.
/// - `entries` — live slice from `ToastStackState::tick()`.
/// - `now_ms` — current Unix epoch ms.
///
/// Skips toasts with alpha ≤ 0 and breaks when the next card would fall below
/// `window_height` (mlc: `if y + toast_height > window_height { break }`).
pub fn draw_toast_stack(
    ctx: &mut dyn RenderContext,
    entries: &[ToastEntry],
    window_width: f64,
    window_height: f64,
    now_ms: u64,
) {
    let start_x = window_width - G::TOAST_WIDTH - G::MARGIN;
    let start_y = G::TOP_ANCHOR;

    for (i, entry) in entries.iter().enumerate() {
        let y = start_y + (i as f64) * G::STACK_PITCH;
        if y + G::TOAST_HEIGHT > window_height {
            break; // mlc parity: don't render off-screen
        }

        let alpha = alpha_for(&entry.toast, now_ms);
        if alpha <= 0.0 {
            continue;
        }

        draw_toast_at(ctx, start_x, y, entry, now_ms);
    }
}

// ─── Legacy single-rect draw (kept for callers that already use it) ───────────

use crate::types::Rect;
use super::settings::ToastSettings;

/// Draw a single toast inside an externally-provided `rect`.
///
/// Kept for backward compatibility. New callers should use `draw_toast_stack`.
pub fn draw_toast(
    ctx: &mut dyn RenderContext,
    rect: Rect,
    entry: &ToastEntry,
    _settings: &ToastSettings,
    now_ms: u64,
) {
    draw_toast_at(ctx, rect.x, rect.y, entry, now_ms);
}
