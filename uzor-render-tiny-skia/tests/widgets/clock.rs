//! Golden grid (H1 Brief 10a-1): clock × 4 built-in sets × its own real
//! state set. `ClockView` carries exactly one interaction flag —
//! `hovered` (module doc: "Sense: HOVER only") — so "normal"/"hover" is the
//! widget's entire real state set.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::clock::{self, ClockRenderKind, ClockSettings, ClockView};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(10.0, 12.0, 140.0, 24.0)
}

fn render(set: BuiltinSet, hovered: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(160, 48, set);
    let settings = ClockSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    let view = ClockView { time_text: "14:35:22", hovered };
    clock::draw_clock(&mut ctx, rect(), WidgetState::Normal, &view, &settings, &ClockRenderKind::Toolbar);
    ctx
}

/// `(state name, hovered)` — clock's entire real state set (module doc).
const STATES: &[(&str, bool)] = &[("normal", false), ("hover", true)];

#[test]
fn clock_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, hovered) in STATES {
            let ctx = render(set, hovered);
            support::golden("clock", set, state_name, &ctx).expect("clock golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("clock");
}
