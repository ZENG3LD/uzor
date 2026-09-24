//! Golden grid (H1 Brief 10a-1): close_button × 4 built-in sets × its own
//! real state set. `CloseButtonView` carries exactly one interaction flag —
//! `hovered` — so "normal"/"hover" is the widget's entire real state set
//! (matches the brief's own naming: "close_button (normal, hover)").

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::close_button::{
    self, CloseButtonRenderKind, CloseButtonSettings, CloseButtonView,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(15.0, 15.0, 18.0, 18.0)
}

fn render(set: BuiltinSet, hovered: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(48, 48, set);
    let settings =
        CloseButtonSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    let view = CloseButtonView { hovered };
    close_button::draw_close_button(
        &mut ctx,
        rect(),
        WidgetState::Normal,
        &view,
        &settings,
        &CloseButtonRenderKind::Default,
    );
    ctx
}

/// `(state name, hovered)` — close_button's entire real state set (module doc).
const STATES: &[(&str, bool)] = &[("normal", false), ("hover", true)];

#[test]
fn close_button_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered) in STATES {
            let ctx = render(set, hovered);
            support::golden("close_button", set, state_name, &ctx).expect("close_button golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("close_button");
}
