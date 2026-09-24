//! Golden grid (H1 Brief 10a-4): toggle × 4 built-in sets × its own real
//! state set. `ToggleView` carries exactly `toggled`/`label`/`disabled` — off,
//! on, and disabled are the widget's whole meaningful state set for the
//! `Switch` render kind (`IconSwap`/`Custom`/`SwitchWide` are separate
//! variants, not additional states of this one). `draw_toggle_switch_inner`
//! never calls its `draw_icon` closure (the `Switch`/`SwitchWide` arms both
//! route to it and it is `_draw_icon`-prefixed, unused) — the no-op closure
//! below is never invoked, only present to satisfy the generic signature.

#![cfg(feature = "golden")]

use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetState};
use uzor::ui::widgets::atomic::toggle::{self, ToggleRenderKind, ToggleSettings, ToggleView};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

fn rect() -> Rect {
    Rect::new(8.0, 9.0, 44.0, 22.0)
}

fn settings(set: BuiltinSet) -> ToggleSettings {
    ToggleSettings { theme: Box::new(TokenTheme::new(Tokens::builtin(set))), ..ToggleSettings::default() }
}

/// `(state name, toggled, disabled)` — named after `ToggleView`'s own
/// fields, the widget's whole meaningful state set for `Switch` (module doc).
const STATES: &[(&str, bool, bool)] = &[
    ("off", false, false),
    ("on", true, false),
    ("disabled", false, true),
];

fn render(set: BuiltinSet, toggled: bool, disabled: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(160, 40, set);
    let settings = settings(set);
    let view = ToggleView { toggled, label: Some("Enable"), disabled };
    let state = if disabled { WidgetState::Disabled } else { WidgetState::Normal };
    toggle::draw_toggle(
        &mut ctx,
        rect(),
        state,
        &view,
        &settings,
        &ToggleRenderKind::Switch,
        |_ctx, _icon, _rect, _color| {},
    );
    ctx
}

#[test]
fn toggle_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, toggled, disabled) in STATES {
            let ctx = render(set, toggled, disabled);
            support::golden("toggle", set, state_name, &ctx).expect("toggle golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("toggle");
}
