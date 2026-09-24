//! Golden grid (H1 Brief 10a-3): text × 4 built-in sets × its own real state
//! set. `TextView::hovered` only drives colour when `view.color` is `None`
//! (`draw_text`'s own doc: falls back to `theme.text_color_hover()` vs
//! `theme.text_color()`) — with no `color` override, `hovered` is the
//! widget's entire real state axis, so "normal"/"hover" is its complete
//! state set (no third state exists on this trait or view).
//!
//! NOTE: `TextTokens`' default table aliases `text_color` to
//! `Role::TextPrimary` and `text_color_hover` to `Role::TextOnAccent`
//! (`tokens.rs`) — two different roles — but every built-in JSON
//! (`dark`/`light`/`high_contrast`/`high_contrast_mono`) defines
//! `color.text.on_accent` as `{color.text.primary}`, so the two resolve to
//! the identical string in all 4 sets today. The "normal" and "hover"
//! golden cells are therefore pixel-identical in every set (verified byte-
//! for-byte, not just by eye) — a token-table fact across all 4 built-in
//! JSON files, not a bug in this test or in `draw_text`; not fixed here per
//! this brief's item 1 scope (no token changes).

#![cfg(feature = "golden")]

use uzor::render::{TextAlign, TextBaseline};
use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::text::{self, TextOverflow, TextSettings, TextView};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(4.0, 4.0, 112.0, 24.0)
}

fn render(set: BuiltinSet, hovered: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(120, 32, set);
    let settings = TextSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    let view = TextView {
        text: "Sample label",
        align: TextAlign::Left,
        baseline: TextBaseline::Middle,
        color: None,
        font: None,
        overflow: TextOverflow::Clip,
        hovered,
    };
    text::draw_text(&mut ctx, rect(), &view, &settings);
    ctx
}

/// `(state name, hovered)` — text's entire real state set (module doc).
const STATES: &[(&str, bool)] = &[("normal", false), ("hover", true)];

#[test]
fn text_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered) in STATES {
            let ctx = render(set, hovered);
            support::golden("text", set, state_name, &ctx).expect("text golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("text");
}
