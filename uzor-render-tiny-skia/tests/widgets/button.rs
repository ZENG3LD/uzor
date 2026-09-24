//! Widget harness+golden+a11y round trip for button (H1 Brief 10a-1: migrated
//! off H0's single white-canvas 3-state set onto the full golden grid —
//! `button/<set>/{default,hover,pressed,disabled}.png` for all 4 built-in
//! token sets, each drawn with its own `TokenTheme`). Also keeps H0's click
//! through the real `TestHarness`/`InputCoordinator` lifecycle and an
//! accessibility query (role=Button, name="Save", has `Click` action).

#![cfg(feature = "golden")]

use uzor::a11y::{A11yAction, A11yRole};
use uzor::input::pointer::state::MouseButton;
use uzor::input::LayerId;
use uzor::testing::{A11yQuery, TestHarness};
use uzor::tokens::{BuiltinSet, TokenTheme, Tokens};
use uzor::types::{Rect, WidgetId, WidgetState};
use uzor::ui::widgets::atomic::button::{self, ButtonSettings, ButtonView};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

use super::support::{self, ReviewSheet};

const LABEL: &str = "Save";

fn rect() -> Rect {
    Rect::new(20.0, 20.0, 120.0, 36.0)
}

fn view(disabled: bool) -> ButtonView<'static> {
    ButtonView {
        icon: None,
        text: Some(LABEL),
        active: false,
        disabled,
        active_border: None,
        hover_chevron: None,
    }
}

fn render(set: BuiltinSet, state: WidgetState, disabled: bool) -> TinySkiaCpuRenderContext {
    let mut ctx = support::canvas(160, 76, set);
    let settings = ButtonSettings::default().with_theme(Box::new(TokenTheme::new(Tokens::builtin(set))));
    button::draw_button(&mut ctx, rect(), state, &view(disabled), &settings, |_, _, _, _| {});
    ctx
}

/// `(state name, coordinator state, view.disabled)` — the 4 states H1's
/// golden grid design doc §5 lists for button.
const STATES: &[(&str, WidgetState, bool)] = &[
    ("default", WidgetState::Normal, false),
    ("hover", WidgetState::Hovered, false),
    ("pressed", WidgetState::Pressed, false),
    ("disabled", WidgetState::Disabled, true),
];

#[test]
fn button_matches_golden_grid() {
    let mut sheet = ReviewSheet::new();
    support::for_each_set(|set, _tokens| {
        for &(state_name, state, disabled) in STATES {
            let ctx = render(set, state, disabled);
            support::golden("button", set, state_name, &ctx).expect("button golden mismatch");
            sheet.add(set, state_name, &ctx);
        }
    });
    sheet.save("button");
}

#[test]
fn button_click_updates_response_and_a11y_node() {
    let id = WidgetId::from("demo-button");
    let mut harness = TestHarness::new();
    let (cx, cy) = (rect().x + 10.0, rect().y + 10.0);

    // Frame 1 — first-ever registration. Establishes the widget for the
    // coordinator's NEXT-frame hover/press bake (see
    // `uzor::testing::TestHarness`'s own struct-level doc on the warm-up-
    // frame caveat: a widget registered for the first time inside a
    // `frame()` call is never Hovered/Pressed in that SAME call) and
    // proves the a11y node exists and is correct before any input at all.
    let (_, f1) = harness.frame(|coord, a11y| {
        button::register(coord, id.clone(), rect(), &LayerId::main());
        a11y.push(button::node_for(id.clone(), rect(), &view(false), false));
    });
    let node = A11yQuery::new(&f1.a11y)
        .find_by_role_name(A11yRole::Button, LABEL)
        .expect("button a11y node must exist on frame 1");
    assert_eq!(node.role, A11yRole::Button);
    assert_eq!(node.name, LABEL);
    assert!(node.actions.contains(&A11yAction::Click));
    assert!(!node.disabled);

    // Frame 2 — pointer down inside the button. Frame 1 already
    // registered this widget, so `begin_frame`'s hit-test bake (against
    // frame 1's registration) sees it immediately — this is NOT the
    // widget's first-time-registered frame, so no extra warm-up frame is
    // needed here. Proves the H0 press-bake fix
    // (`InputCoordinator::begin_frame` baking `mouse_pressed`) is live
    // end-to-end through the public API, not just the crate's own private
    // unit tests.
    harness.events.pointer_down(cx, cy, MouseButton::Left);
    harness.frame(|coord, a11y| {
        button::register(coord, id.clone(), rect(), &LayerId::main());
        a11y.push(button::node_for(id.clone(), rect(), &view(false), false));
    });
    assert_eq!(harness.coordinator.widget_state(&id), WidgetState::Pressed);

    // Frame 3 — pointer up at the same point: `end_frame`'s own
    // THIS-frame hit test (not the `begin_frame` bake) reports `clicked`
    // straight off this frame's registration.
    harness.events.pointer_up(cx, cy, MouseButton::Left);
    let (_, f3) = harness.frame(|coord, a11y| {
        button::register(coord, id.clone(), rect(), &LayerId::main());
        a11y.push(button::node_for(id.clone(), rect(), &view(false), false));
    });
    let (_, resp) = f3
        .responses
        .iter()
        .find(|(wid, _)| *wid == id)
        .expect("button response must exist");
    assert!(resp.clicked, "button must report clicked on the pointer-up frame");

    let node3 = A11yQuery::new(&f3.a11y)
        .find_by_role_name(A11yRole::Button, LABEL)
        .expect("button a11y node must exist on frame 3");
    assert_eq!(node3.role, A11yRole::Button);
}
