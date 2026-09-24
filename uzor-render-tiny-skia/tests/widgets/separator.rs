//! Golden grid (H1 Brief 10a-3): separator × 4 built-in sets × its own real
//! state set, using the generic `draw_separator` + `DefaultSeparatorStyle`
//! (1 px line, no margin — `style.rs`'s own default preset). `SeparatorView`
//! carries `kind`/`hovered`/`dragging`; `draw_separator`'s own colour match
//! only distinguishes `ResizeHandle` from `Divider`, and within
//! `ResizeHandle` only `dragging` from `hovered` from neither — so "line"
//! (plain `Divider`, `theme.line()`), "handle_hover" (`ResizeHandle`,
//! hovered, `theme.handle_hover()`), and "handle_active" (`ResizeHandle`,
//! dragging, `theme.handle_active()`) is the widget's entire real colour
//! axis for this one function (module doc on `render.rs`'s function map —
//! the other 5 `draw_*` functions are separate visual patterns, out of
//! scope for a single grid).

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::Rect;
use uzor::ui::widgets::atomic::separator::{
    self, DefaultSeparatorStyle, SeparatorSettings, SeparatorType, SeparatorView,
};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(4.0, 4.0, 120.0, 16.0)
}

enum State {
    Line,
    HandleHover,
    HandleActive,
}

fn render(set: BuiltinSet, state: &State) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(128, 24, set);
    let settings = SeparatorSettings {
        theme: Box::new(TokenTheme::new(Tokens::builtin(set))),
        style: Box::new(DefaultSeparatorStyle),
    };
    let view = match state {
        State::Line => SeparatorView {
            kind: SeparatorType::horizontal_divider(),
            hovered: false,
            dragging: false,
        },
        State::HandleHover => SeparatorView {
            kind: SeparatorType::horizontal_resize(),
            hovered: true,
            dragging: false,
        },
        State::HandleActive => SeparatorView {
            kind: SeparatorType::horizontal_resize(),
            hovered: false,
            dragging: true,
        },
    };
    separator::draw_separator(&mut ctx, rect(), &view, &settings);
    ctx
}

/// `(state name, state)` — the entire colour axis `draw_separator` itself
/// distinguishes (module doc).
const STATES: &[(&str, State)] = &[
    ("line", State::Line),
    ("handle_hover", State::HandleHover),
    ("handle_active", State::HandleActive),
];

#[test]
fn separator_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for (state_name, state) in STATES {
            let ctx = render(set, state);
            support::golden("separator", set, state_name, &ctx)
                .expect("separator golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("separator");
}
