//! Tooltip guide — a small measured key/value box anchored near a pixel
//! position, flipping to the opposite side when it would run off the
//! given bounds. Idiom generalized from `mylittlechart`'s
//! `chart_render/overlays.rs::draw_tooltip` +
//! `Crosshair::get_tooltip_position[_at]` edge-flip math — OHLC-specific
//! tooltip content dropped; this crate's tooltip is a generic key/value
//! row list, content supplied entirely by the caller (a figure's own
//! `render_with`).
//!
//! Deviates from a literal port in one way worth flagging: mlc's
//! `get_tooltip_position[_at]` flips against `chart_width`/`chart_height`
//! (chart-local, always starting at `0`); this crate's figures can sit
//! anywhere in a larger canvas (multi-panel layouts), so this version
//! takes an explicit `bounds: Rect` to flip against instead of an
//! implicit width/height pair.

use uzor::render::{RenderContext, TextAlign, TextBaseline};
use uzor::types::Rect;

use crate::theme::FigureTheme;

const PAD: f64 = 8.0;
const ROW_GAP: f64 = 2.0;
const KEY_VALUE_GAP: f64 = 12.0;
const CURSOR_OFFSET: f64 = 12.0;
const CORNER_RADIUS: f64 = 4.0;
const TOOLTIP_ALPHA: f64 = 0.94;

/// Draw a `lines`-row tooltip box near `anchor_px`, staying inside
/// `bounds` (flips left instead of right, and clamps vertically, when it
/// would otherwise overflow). No-op for empty `lines`.
pub fn draw_tooltip(ctx: &mut dyn RenderContext, theme: &FigureTheme, anchor_px: (f64, f64), lines: &[(String, String)], bounds: Rect) {
    if lines.is_empty() {
        return;
    }

    // Guide-state hygiene: this fn flips text align (Right for the value
    // column) and baseline (Middle) — without a save/restore bracket that
    // state LEAKED into whatever the caller painted next (live-caught
    // 2026-07-24: a graph demo's whole HUD text shifted left by each
    // string's own width whenever the hover tooltip was open, because
    // every later `fill_text` inherited `TextAlign::Right`). `save`/
    // `restore` stacks text align/baseline in the render backends, so the
    // caller gets its own state back regardless of what this box drew.
    ctx.save();
    ctx.set_font(&theme.label_font);
    let row_h = ctx.text_bounds("Ag", &theme.label_font).h.max(12.0) + ROW_GAP;
    let key_w = lines.iter().map(|(k, _)| ctx.measure_text(k)).fold(0.0_f64, f64::max);
    let val_w = lines.iter().map(|(_, v)| ctx.measure_text(v)).fold(0.0_f64, f64::max);
    let box_w = PAD * 2.0 + key_w + KEY_VALUE_GAP + val_w;
    let box_h = PAD * 2.0 + row_h * lines.len() as f64;

    let (anchor_x, anchor_y) = anchor_px;
    let mut box_x = anchor_x + CURSOR_OFFSET;
    if box_x + box_w > bounds.right() {
        box_x = (anchor_x - CURSOR_OFFSET - box_w).max(bounds.x);
    }
    let mut box_y = anchor_y - box_h / 2.0;
    if box_y < bounds.y {
        box_y = bounds.y;
    }
    if box_y + box_h > bounds.bottom() {
        box_y = (bounds.bottom() - box_h).max(bounds.y);
    }

    ctx.set_fill_color(&theme.background);
    ctx.set_global_alpha(TOOLTIP_ALPHA);
    ctx.fill_rounded_rect(box_x, box_y, box_w, box_h, CORNER_RADIUS);
    ctx.set_global_alpha(1.0);

    ctx.set_stroke_color(&theme.axis_color);
    ctx.set_stroke_width(1.0);
    ctx.stroke_rounded_rect(box_x, box_y, box_w, box_h, CORNER_RADIUS);

    ctx.set_font(&theme.label_font);
    ctx.set_text_baseline(TextBaseline::Middle);
    for (i, (key, value)) in lines.iter().enumerate() {
        let row_center_y = box_y + PAD + row_h * i as f64 + row_h / 2.0;
        ctx.set_fill_color(&theme.label_color);
        ctx.set_text_align(TextAlign::Left);
        ctx.fill_text(key, box_x + PAD, row_center_y);
        ctx.set_text_align(TextAlign::Right);
        ctx.fill_text(value, box_x + box_w - PAD, row_center_y);
    }
    // Belt-and-suspenders for any backend whose save/restore doesn't
    // stack text state: land on the workspace-default alignment
    // explicitly before restoring.
    ctx.set_text_align(TextAlign::Left);
    ctx.restore();
}
