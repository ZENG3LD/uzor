//! Golden grid (H1 Brief 10a-4): tooltip × 4 built-in sets × its own real
//! default state. `draw_tooltip` (generic, back-compat) is the render path
//! actually wired to the widget's input registration
//! (`register_context_manager_tooltip` calls it directly, `input.rs`) — the
//! other two entry points, `draw_chrome_tooltip` and `draw_crosshair_tooltip`,
//! are separate MLC parity variants used by other call sites (`uzor-figures`
//! guides), not this atomic widget's own registered path.
//!
//! `draw_tooltip` draws bg + text always, plus a border stroke whenever
//! `style.border_width() > 0` (`DefaultTooltipStyle` = `1.0`) — it has no
//! shadow branch at all (`ChromeTooltipStyle::has_shadow` only gates
//! `draw_chrome_tooltip`, a function this golden does not call, and that
//! variant's own doc says it has no border stroke either: "Chrome variant
//! has no border stroke — shadow only"). So "default" here is text + border
//! at full opacity — the widget's real, whole capability through its actual
//! registered draw fn, not an invented shadow no real call site produces
//! together with a border.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::tooltip::{self, DefaultTooltipStyle, TooltipConfig, TooltipSettings};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(8.0, 8.0, 120.0, 24.0)
}

fn settings(set: BuiltinSet) -> TooltipSettings {
    TooltipSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::new(DefaultTooltipStyle),
    }
}

/// Single meaningful state (module doc): `draw_tooltip`'s only axis is
/// `alpha` (fade), and this golden fixes it to `1.0` (fully opaque) — the
/// widget declares no other interaction-relevant field on this render path.
const STATES: &[&str] = &["default"];

fn render(set: BuiltinSet) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(136, 40, set);
    let settings = settings(set);
    let anchor = Rect::new(0.0, 0.0, 0.0, 0.0);
    let config = TooltipConfig::below("Tooltip text", anchor);
    tooltip::draw_tooltip(&mut ctx, rect(), &config, 1.0, &settings);
    ctx
}

#[test]
fn tooltip_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &state_name in STATES {
            let ctx = render(set);
            support::golden("tooltip", set, state_name, &ctx).expect("tooltip golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("tooltip");
}
