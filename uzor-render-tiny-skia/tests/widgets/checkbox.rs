//! Golden grid (H1 Brief 10a-1): checkbox × 4 built-in sets × its own real
//! state set. `draw_checkbox`'s `Standard` variant has exactly one
//! interaction-relevant field on `CheckboxView` — `checked` — and
//! `CheckboxTheme` carries no hover/disabled colour slots at all (no
//! `checkbox_bg_hover`/`checkbox_bg_disabled` method exists), so "unchecked"
//! and "checked" are the widget's entire real state set; inventing a
//! hover/disabled pair here would test states the widget cannot render.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::checkbox::{self, CheckboxRenderKind, CheckboxSettings, CheckboxView};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

const FONT: &str = "13px sans-serif";

fn rect() -> Rect {
    Rect::new(20.0, 20.0, 16.0, 16.0)
}

fn render(set: BuiltinSet, checked: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(160, 56, set);
    let settings = CheckboxSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    let view = CheckboxView { checked, label: Some("Label") };
    checkbox::draw_checkbox(
        &mut ctx,
        rect(),
        WidgetState::Normal,
        &view,
        &settings,
        &CheckboxRenderKind::Standard,
        FONT,
    );
    ctx
}

/// `(state name, checked)` — checkbox's entire real state set (module doc).
const STATES: &[(&str, bool)] = &[("unchecked", false), ("checked", true)];

#[test]
fn checkbox_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, checked) in STATES {
            let ctx = render(set, checked);
            support::golden("checkbox", set, state_name, &ctx).expect("checkbox golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("checkbox");
}
