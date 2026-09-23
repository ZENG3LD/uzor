//! First widget harness+golden+a11y round trip (H0 done-criterion):
//! button passes three golden PNG states (default/hover/pressed), a
//! click through the real `TestHarness`/`InputCoordinator` lifecycle, and
//! an accessibility query (role=Button, name="Save", has `Click` action).

#![cfg(feature = "golden")]

use tiny_skia::Color;
use uzor::a11y::{A11yAction, A11yRole};
use uzor::input::pointer::state::MouseButton;
use uzor::input::LayerId;
use uzor::testing::{A11yQuery, TestHarness};
use uzor::types::{Rect, WidgetId, WidgetState};
use uzor::ui::widgets::atomic::button::{self, ButtonSettings, ButtonView};
use uzor_render_tiny_skia::golden::{compare_or_bless, GoldenTolerance};
use uzor_render_tiny_skia::TinySkiaCpuRenderContext;

const LABEL: &str = "Save";

fn rect() -> Rect {
    Rect::new(20.0, 20.0, 120.0, 36.0)
}

fn view() -> ButtonView<'static> {
    ButtonView {
        icon: None,
        text: Some(LABEL),
        active: false,
        disabled: false,
        active_border: None,
        hover_chevron: None,
    }
}

fn render(state: WidgetState) -> TinySkiaCpuRenderContext {
    let mut ctx = TinySkiaCpuRenderContext::new(160, 76, 1.0);
    ctx.clear(Color::WHITE);
    button::draw_button(&mut ctx, rect(), state, &view(), &ButtonSettings::default(), |_, _, _, _| {});
    ctx
}

#[test]
fn button_default_matches_golden() {
    let ctx = render(WidgetState::Normal);
    compare_or_bless("button/default", ctx.pixels(), ctx.width(), ctx.height(), GoldenTolerance::default())
        .expect("button/default golden mismatch");
}

#[test]
fn button_hover_matches_golden() {
    let ctx = render(WidgetState::Hovered);
    compare_or_bless("button/hover", ctx.pixels(), ctx.width(), ctx.height(), GoldenTolerance::default())
        .expect("button/hover golden mismatch");
}

#[test]
fn button_pressed_matches_golden() {
    let ctx = render(WidgetState::Pressed);
    compare_or_bless("button/pressed", ctx.pixels(), ctx.width(), ctx.height(), GoldenTolerance::default())
        .expect("button/pressed golden mismatch");
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
        a11y.push(button::node_for(id.clone(), rect(), &view(), false));
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
        a11y.push(button::node_for(id.clone(), rect(), &view(), false));
    });
    assert_eq!(harness.coordinator.widget_state(&id), WidgetState::Pressed);

    // Frame 3 — pointer up at the same point: `end_frame`'s own
    // THIS-frame hit test (not the `begin_frame` bake) reports `clicked`
    // straight off this frame's registration.
    harness.events.pointer_up(cx, cy, MouseButton::Left);
    let (_, f3) = harness.frame(|coord, a11y| {
        button::register(coord, id.clone(), rect(), &LayerId::main());
        a11y.push(button::node_for(id.clone(), rect(), &view(), false));
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
