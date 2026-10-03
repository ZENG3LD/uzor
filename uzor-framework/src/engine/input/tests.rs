//! InputEngine tests. Pointer / key routing is driven the way the kernel
//! will drive it: cook (step 1) then, if nothing claimed it, deliver to
//! content (step 5); key step 1 in the InputEngine, step 3 in the
//! KeymapEngine; compose frames as BeginFrame -> registration -> EndFrame.

use super::*;
use crate::engine::keymap::KeymapEngine;
use crate::types::command::{Binding, KeymapCmd, KeymapScope};
use crate::types::ids::OverlaySlot;
use crate::types::ops::{EngineTarget, KeymapOp};
use crate::types::window::Point;
use uzor::input::core::cook::DRAG_THRESHOLD_PX;
use uzor::input::{KeyboardShortcut, Sense, TextFieldConfig};

const W: WindowId = WindowId(1);
const L: MouseButton = MouseButton::Left;
const CHAR_W: f64 = 8.0;

fn s(t: f64) -> Seconds {
    Seconds(t)
}

fn id(name: &str) -> WidgetId {
    WidgetId::from(name)
}

fn engine() -> InputEngine {
    let mut e = InputEngine::new();
    e.apply(InputOp::Open(W));
    e
}

fn no_mods() -> ModifierKeys {
    ModifierKeys::default()
}

fn ctrl() -> ModifierKeys {
    ModifierKeys {
        ctrl: true,
        ..ModifierKeys::default()
    }
}

fn shift() -> ModifierKeys {
    ModifierKeys {
        shift: true,
        ..ModifierKeys::default()
    }
}

fn down(x: f64, y: f64) -> PointerInput {
    PointerInput::Down {
        pos: Point::new(x, y),
        button: L,
        mods: no_mods(),
    }
}

fn up(x: f64, y: f64) -> PointerInput {
    PointerInput::Up {
        pos: Point::new(x, y),
        button: L,
        mods: no_mods(),
    }
}

fn moved(x: f64, y: f64) -> PointerInput {
    PointerInput::Moved {
        pos: Point::new(x, y),
        mods: no_mods(),
    }
}

/// Routing step 1 only.
fn cook(e: &mut InputEngine, t: f64, input: PointerInput) -> Option<CookedPointer> {
    e.apply(InputOp::Pointer {
        win: W,
        now: s(t),
        input,
    })
    .into_iter()
    .find_map(|f| match f {
        InputEffect::Pointer(c) => Some(c),
        _ => None,
    })
}

/// Steps 1 and 5: nothing between claims the event.
fn to_content(e: &mut InputEngine, t: f64, input: PointerInput) -> Option<WidgetId> {
    cook(e, t, input);
    e.apply(InputOp::Deliver { win: W, input })
        .into_iter()
        .find_map(|f| match f {
            InputEffect::Content { target, .. } => target,
            _ => None,
        })
}

fn click(e: &mut InputEngine, t: f64, x: f64, y: f64) {
    to_content(e, t, down(x, y));
    to_content(e, t + 0.02, up(x, y));
}

/// One compose frame; `f` registers widgets.
fn frame(e: &mut InputEngine, t: f64, f: impl FnOnce(&mut InputCoordinator)) -> InputEffects {
    e.apply(InputOp::BeginFrame { win: W, now: s(t) });
    if let Some(c) = e.registrar(W) {
        f(c);
    }
    e.apply(InputOp::EndFrame { win: W, now: s(t) })
}

/// Register a text field and stamp its geometry: one char every `CHAR_W`
/// px from the rect's left edge.
fn field(c: &mut InputCoordinator, name: &str, rect: Rect, config: TextFieldConfig) {
    let fid = id(name);
    c.register_text_field(fid.clone(), rect, config);
    let n = c.text_fields().text(&fid).chars().count();
    let positions = (0..=n).map(|i| rect.x + i as f64 * CHAR_W).collect();
    c.text_fields_mut()
        .update_field(&fid, (rect.x, rect.y, rect.width, rect.height), positions);
}

fn rect_a() -> Rect {
    Rect::new(10.0, 10.0, 200.0, 20.0)
}

fn rect_b() -> Rect {
    Rect::new(10.0, 50.0, 200.0, 20.0)
}

/// A window with text fields `a` and `b` registered.
fn with_fields() -> InputEngine {
    let mut e = engine();
    frame(&mut e, 0.5, two_fields);
    e
}

fn two_fields(c: &mut InputCoordinator) {
    field(c, "a", rect_a(), TextFieldConfig::text());
    field(c, "b", rect_b(), TextFieldConfig::text());
}

fn focus(e: &mut InputEngine, t: f64, op: FocusOp) -> InputEffects {
    e.apply(InputOp::Focus {
        win: W,
        now: s(t),
        op,
    })
}

fn key_input(code: KeyCode, text: Option<&str>, mods: ModifierKeys) -> KeyInput {
    KeyInput {
        code,
        text: text.map(str::to_owned),
        state: KeyState::Down,
        mods,
    }
}

fn key(e: &mut InputEngine, t: f64, code: KeyCode, mods: ModifierKeys) -> InputEffects {
    e.apply(InputOp::Key {
        win: W,
        now: s(t),
        input: key_input(code, None, mods),
    })
}

/// Type `text` one key event per char.
fn typ(e: &mut InputEngine, t: f64, text: &str) -> InputEffects {
    let mut all = InputEffects::new();
    for ch in text.chars() {
        let mut buf = [0u8; 4];
        all.extend(e.apply(InputOp::Key {
            win: W,
            now: s(t),
            input: key_input(KeyCode::A, Some(ch.encode_utf8(&mut buf)), no_mods()),
        }));
    }
    all
}

fn text(e: &InputEngine, name: &str) -> String {
    e.view()
        .text_fields(W)
        .map(|t| t.text(&id(name)).to_owned())
        .unwrap_or_default()
}

fn passed(fx: &InputEffects) -> bool {
    fx.iter()
        .any(|f| matches!(f, InputEffect::KeyPassed { .. }))
}

fn deadlines(fx: &InputEffects) -> Vec<Option<Seconds>> {
    fx.iter()
        .filter_map(|f| match f {
            InputEffect::CaretDeadline { at, .. } => Some(*at),
            _ => None,
        })
        .collect()
}

fn invalidated(fx: &InputEffects) -> Vec<Option<Rect>> {
    fx.iter()
        .filter_map(|f| match f {
            InputEffect::InvalidateField { rect, .. } => Some(*rect),
            _ => None,
        })
        .collect()
}

fn changed_fields(fx: &InputEffects) -> Vec<WidgetId> {
    fx.iter()
        .filter_map(|f| match f {
            InputEffect::Text(TextIntent::Changed { field, .. }) => Some(field.clone()),
            _ => None,
        })
        .collect()
}

fn copied(fx: &InputEffects) -> Vec<String> {
    fx.iter()
        .filter_map(|f| match f {
            InputEffect::CopyText { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn overlay(n: u64) -> ScopeOwner {
    ScopeOwner::Overlay(OverlaySlot(n))
}

fn push(e: &mut InputEngine, t: f64, owner: ScopeOwner, members: &[&str]) -> InputEffects {
    focus(
        e,
        t,
        FocusOp::PushScope {
            owner,
            members: members.iter().map(|m| id(m)).collect(),
        },
    )
}

// ---------------------------------------------------------------------------
// Cook
// ---------------------------------------------------------------------------

#[test]
fn cook_counts_double_and_triple_clicks_and_resets() {
    let mut e = engine();
    let mut counts = Vec::new();
    for (t, x) in [(1.0, 10.0), (1.2, 11.0), (1.4, 12.0), (1.6, 12.0)] {
        counts.push(cook(&mut e, t, down(x, 10.0)).map(|c| c.click_count));
        cook(&mut e, t + 0.05, up(x, 10.0));
    }
    // Fourth quick click stays a triple.
    assert_eq!(counts, vec![Some(1), Some(2), Some(3), Some(3)]);
    // Too far away: a new sequence.
    let far = cook(&mut e, 1.8, down(40.0, 10.0)).map(|c| c.click_count);
    cook(&mut e, 1.85, up(40.0, 10.0));
    assert_eq!(far, Some(1));
    // Too late: a new sequence.
    let late = cook(&mut e, 3.0, down(40.0, 10.0)).map(|c| c.click_count);
    assert_eq!(late, Some(1));
    // The release keeps the count of its press.
    assert_eq!(
        cook(&mut e, 3.05, up(40.0, 10.0)).map(|c| c.click_count),
        Some(1)
    );
}

#[test]
fn cook_arms_a_drag_past_the_threshold() {
    let mut e = engine();
    cook(&mut e, 1.0, down(10.0, 10.0));
    let near = cook(&mut e, 1.1, moved(10.0 + DRAG_THRESHOLD_PX - 1.0, 10.0));
    assert_eq!(
        near.as_ref().map(|c| (c.drag_armed, c.drag_started)),
        Some((false, false))
    );
    let far = cook(&mut e, 1.2, moved(10.0 + DRAG_THRESHOLD_PX, 10.0));
    assert_eq!(
        far.as_ref().map(|c| (c.drag_armed, c.drag_started)),
        Some((true, true))
    );
    let further = cook(&mut e, 1.3, moved(40.0, 10.0));
    assert_eq!(
        further.as_ref().map(|c| (c.drag_armed, c.drag_started)),
        Some((true, false))
    );
    let release = cook(&mut e, 1.4, up(40.0, 10.0));
    assert_eq!(release.as_ref().map(|c| c.ended_drag), Some(true));
    assert_eq!(e.view().cook(W).map(|c| c.drag_armed), Some(false));
}

/// The coordinator's own cook, fed with the stamped frame time, makes
/// `WidgetResponse.double_clicked / triple_clicked` real.
#[test]
fn clicks_through_the_coordinator_carry_the_click_count() {
    let mut e = engine();
    let button =
        |c: &mut InputCoordinator| c.register("btn", Rect::new(0.0, 0.0, 50.0, 20.0), Sense::CLICK);
    frame(&mut e, 0.5, button);
    let mut seen = Vec::new();
    for t in [1.0, 1.2, 1.4, 3.0] {
        click(&mut e, t, 10.0, 10.0);
        frame(&mut e, t + 0.05, button);
        seen.push(e.view().clicked(W).to_vec());
    }
    let btn = id("btn");
    assert_eq!(
        seen,
        vec![
            vec![(btn.clone(), 1)],
            vec![(btn.clone(), 2)],
            vec![(btn.clone(), 3)],
            vec![(btn.clone(), 1)],
        ]
    );
    // Hover follows the pointer; clicks last one frame.
    assert_eq!(e.view().window(W).and_then(|v| v.hovered), Some(btn));
    frame(&mut e, 3.2, button);
    assert!(e.view().clicked(W).is_empty());
}

#[test]
fn press_and_hover_per_window() {
    let mut e = engine();
    let button =
        |c: &mut InputCoordinator| c.register("btn", Rect::new(0.0, 0.0, 50.0, 20.0), Sense::CLICK);
    frame(&mut e, 0.5, button);
    assert_eq!(to_content(&mut e, 1.0, moved(10.0, 10.0)), Some(id("btn")));
    frame(&mut e, 1.0, button);
    assert_eq!(e.view().window(W).and_then(|v| v.hovered), Some(id("btn")));
    assert_eq!(to_content(&mut e, 1.1, down(10.0, 10.0)), Some(id("btn")));
    assert_eq!(e.view().window(W).and_then(|v| v.pressed), Some(id("btn")));
    to_content(&mut e, 1.2, up(10.0, 10.0));
    assert_eq!(e.view().window(W).and_then(|v| v.pressed), None);
    // Nothing under the pointer: unclaimed (routing step 6).
    assert_eq!(to_content(&mut e, 1.3, moved(300.0, 300.0)), None);
    frame(&mut e, 1.3, button);
    assert_eq!(e.view().window(W).and_then(|v| v.hovered), None);
    // Another window has its own state.
    e.apply(InputOp::Open(WindowId(2)));
    assert_eq!(
        e.view()
            .window(WindowId(2))
            .map(|v| (v.hovered, v.pressed, v.focused)),
        Some((None, None, None))
    );
}

// ---------------------------------------------------------------------------
// Capture
// ---------------------------------------------------------------------------

#[test]
fn engine_capture_routes_regardless_of_position_until_release() {
    let mut e = engine();
    let splitter = Capture::Engine(EngineTarget::Layout);
    cook(&mut e, 1.0, down(100.0, 100.0));
    e.apply(InputOp::SetCapture {
        win: W,
        capture: Some(splitter.clone()),
    });
    assert_eq!(e.view().window(W).map(|v| v.captured), Some(true));
    // Anywhere, even over another overlay's body: the owner gets it.
    for (x, y) in [(500.0, 5.0), (-40.0, 900.0)] {
        let c = cook(&mut e, 1.1, moved(x, y));
        assert_eq!(
            c.map(|c| c.target),
            Some(PointerTarget::Captured(splitter.clone()))
        );
    }
    // The release still goes to the owner, then the capture ends.
    let release = cook(&mut e, 1.2, up(500.0, 5.0));
    assert_eq!(
        release.map(|c| c.target),
        Some(PointerTarget::Captured(splitter))
    );
    assert_eq!(e.view().capture(W), None);
    let after = cook(&mut e, 1.3, moved(500.0, 5.0));
    assert_eq!(after.map(|c| c.target), Some(PointerTarget::HitTest));
}

#[test]
fn widget_capture_bypasses_the_hit_test_and_grabs_in_the_coordinator() {
    let mut e = engine();
    let widgets = |c: &mut InputCoordinator| {
        c.register(
            "slider",
            Rect::new(0.0, 0.0, 50.0, 20.0),
            Sense::CLICK_AND_DRAG,
        );
        c.register("other", Rect::new(0.0, 100.0, 50.0, 20.0), Sense::CLICK);
    };
    frame(&mut e, 0.5, widgets);
    to_content(&mut e, 1.0, down(10.0, 10.0));
    e.apply(InputOp::SetCapture {
        win: W,
        capture: Some(Capture::Widget(id("slider"))),
    });
    assert_eq!(
        e.view()
            .coordinator(W)
            .and_then(|c| c.cook_state().grabbed.clone()),
        Some(id("slider"))
    );
    // Over "other", the event still goes to the slider.
    assert_eq!(
        to_content(&mut e, 1.1, moved(10.0, 110.0)),
        Some(id("slider"))
    );
    frame(&mut e, 1.1, widgets);
    assert_eq!(
        e.view().window(W).and_then(|v| v.hovered),
        Some(id("slider"))
    );
    // Release ends it in both places.
    to_content(&mut e, 1.2, up(10.0, 110.0));
    assert_eq!(e.view().capture(W), None);
    assert_eq!(
        e.view()
            .coordinator(W)
            .and_then(|c| c.cook_state().grabbed.clone()),
        None
    );
    assert_eq!(
        to_content(&mut e, 1.3, moved(10.0, 110.0)),
        Some(id("other"))
    );
}

#[test]
fn os_cancel_releases_the_capture() {
    let mut e = engine();
    cook(&mut e, 1.0, down(1.0, 1.0));
    e.apply(InputOp::SetCapture {
        win: W,
        capture: Some(Capture::Engine(EngineTarget::Overlay(OverlaySlot(3)))),
    });
    cook(&mut e, 1.1, PointerInput::Cancelled);
    assert_eq!(e.view().capture(W), None);
    assert_eq!(
        e.view().cook(W).map(|c| c.press_origin.is_none()),
        Some(true)
    );
}

// ---------------------------------------------------------------------------
// Focus scopes
// ---------------------------------------------------------------------------

fn modal_frame(c: &mut InputCoordinator) {
    field(c, "bg", rect_a(), TextFieldConfig::text());
    c.register("m1", Rect::new(300.0, 10.0, 50.0, 20.0), Sense::FOCUSABLE);
    field(
        c,
        "m2",
        Rect::new(300.0, 50.0, 100.0, 20.0),
        TextFieldConfig::text(),
    );
    c.register("m3", Rect::new(300.0, 90.0, 50.0, 20.0), Sense::FOCUSABLE);
}

#[test]
fn scope_push_pop_restores_and_guards_set() {
    let mut e = engine();
    frame(&mut e, 0.5, modal_frame);
    focus(&mut e, 1.0, FocusOp::Set(id("bg")));
    typ(&mut e, 1.0, "hi");
    assert_eq!(e.view().focused(W), Some(&id("bg")));

    // Open the modal: focus is saved and cleared.
    let fx = push(&mut e, 1.1, overlay(1), &["m1", "m2", "m3"]);
    assert!(fx.iter().any(|f| matches!(
        f,
        InputEffect::ScopePushed { owner, .. } if *owner == overlay(1)
    )));
    assert_eq!(e.view().focused(W), None);
    assert_eq!(e.view().active_scope(W), Some(&overlay(1)));
    assert_eq!(
        e.view().text_fields(W).and_then(|t| t.focused().cloned()),
        None
    );

    // Set outside the active scope: a no-op, no revision change.
    let rev = e.revision();
    focus(&mut e, 1.2, FocusOp::Set(id("bg")));
    assert_eq!(e.view().focused(W), None);
    assert_eq!(e.revision(), rev);

    // Set inside works; typing lands in the modal field.
    focus(&mut e, 1.3, FocusOp::Set(id("m2")));
    typ(&mut e, 1.3, "x");
    assert_eq!(text(&e, "m2"), "x");
    assert_eq!(text(&e, "bg"), "hi");

    // Close with restore: the background field is focused and still engaged.
    focus(
        &mut e,
        1.4,
        FocusOp::PopScope {
            owner: overlay(1),
            restore: true,
        },
    );
    assert_eq!(e.view().focused(W), Some(&id("bg")));
    assert_eq!(e.view().active_scope(W), None);
    let store = e.view().text_fields(W);
    assert_eq!(store.and_then(|t| t.focused().cloned()), Some(id("bg")));
    assert_eq!(store.map(|t| t.is_engaged(&id("bg"))), Some(true));
    typ(&mut e, 1.5, "!");
    assert_eq!(text(&e, "bg"), "hi!");

    // Close without restore clears focus instead.
    push(&mut e, 1.6, overlay(2), &["m1"]);
    focus(&mut e, 1.6, FocusOp::Set(id("m1")));
    focus(
        &mut e,
        1.7,
        FocusOp::PopScope {
            owner: overlay(2),
            restore: false,
        },
    );
    assert_eq!(e.view().focused(W), None);
}

#[test]
fn tab_order_is_scoped_to_the_active_scope() {
    let mut e = engine();
    frame(&mut e, 0.5, modal_frame);
    push(&mut e, 1.0, overlay(1), &["m1", "m2", "m3", "gone"]);
    let tab = |e: &mut InputEngine, m: ModifierKeys| {
        let fx = key(e, 1.1, KeyCode::Tab, m);
        assert!(!passed(&fx));
        e.view().focused(W).cloned()
    };
    // Unregistered member "gone" is skipped; "bg" is never reached.
    assert_eq!(tab(&mut e, no_mods()), Some(id("m1")));
    assert_eq!(tab(&mut e, no_mods()), Some(id("m2")));
    // The text field member also takes the text store focus.
    assert_eq!(
        e.view().text_fields(W).and_then(|t| t.focused().cloned()),
        Some(id("m2"))
    );
    assert_eq!(tab(&mut e, no_mods()), Some(id("m3")));
    assert_eq!(tab(&mut e, no_mods()), Some(id("m1")));
    assert_eq!(tab(&mut e, shift()), Some(id("m3")));
    assert_eq!(tab(&mut e, shift()), Some(id("m2")));
    // FocusOp::Next / Prev are the same order.
    focus(&mut e, 1.2, FocusOp::Next);
    assert_eq!(e.view().focused(W), Some(&id("m3")));
    focus(&mut e, 1.2, FocusOp::Prev);
    assert_eq!(e.view().focused(W), Some(&id("m2")));
}

#[test]
fn tab_without_a_scope_cycles_every_focusable_widget() {
    let mut e = engine();
    frame(&mut e, 0.5, modal_frame);
    let mut order = Vec::new();
    for _ in 0..5 {
        key(&mut e, 1.0, KeyCode::Tab, no_mods());
        order.push(e.view().focused(W).cloned());
    }
    let ids = ["bg", "m1", "m2", "m3", "bg"];
    assert_eq!(order, ids.iter().map(|n| Some(id(n))).collect::<Vec<_>>());
    // The text store follows: only the text fields hold it.
    assert_eq!(
        e.view().text_fields(W).and_then(|t| t.focused().cloned()),
        Some(id("bg"))
    );
    // Nothing focusable: Tab routes on (e.g. to a binding).
    let mut empty = engine();
    frame(&mut empty, 0.5, |_| {});
    assert!(passed(&key(&mut empty, 1.0, KeyCode::Tab, no_mods())));
}

#[test]
fn closing_a_lower_scope_hands_its_saved_focus_up() {
    let mut e = engine();
    frame(&mut e, 0.5, modal_frame);
    focus(&mut e, 1.0, FocusOp::Set(id("bg")));
    push(&mut e, 1.1, overlay(1), &["m1", "m2"]);
    focus(&mut e, 1.1, FocusOp::Set(id("m1")));
    push(&mut e, 1.2, overlay(2), &["m3"]);
    // The lower modal closes first (e.g. by command) while the upper one
    // is open: the upper scope must not restore into the closed one.
    focus(
        &mut e,
        1.3,
        FocusOp::PopScope {
            owner: overlay(1),
            restore: true,
        },
    );
    assert_eq!(e.view().active_scope(W), Some(&overlay(2)));
    assert_eq!(e.view().window(W).map(|v| v.scope_depth), Some(1));
    focus(
        &mut e,
        1.4,
        FocusOp::PopScope {
            owner: overlay(2),
            restore: true,
        },
    );
    assert_eq!(e.view().focused(W), Some(&id("bg")));
    // Popping an unknown owner changes nothing.
    let rev = e.revision();
    focus(
        &mut e,
        1.5,
        FocusOp::PopScope {
            owner: overlay(9),
            restore: true,
        },
    );
    assert_eq!(e.revision(), rev);
}

#[test]
fn a_click_into_a_field_outside_the_scope_is_reverted() {
    let mut e = engine();
    frame(&mut e, 0.5, modal_frame);
    push(&mut e, 1.0, overlay(1), &["m2"]);
    focus(&mut e, 1.0, FocusOp::Set(id("m2")));
    // The coordinator's own click handling focuses "bg"; EndFrame reverts.
    click(&mut e, 1.1, 20.0, 15.0);
    frame(&mut e, 1.15, modal_frame);
    assert_eq!(e.view().focused(W), Some(&id("m2")));
    assert_eq!(
        e.view().text_fields(W).and_then(|t| t.focused().cloned()),
        Some(id("m2"))
    );
    // Without a scope the same click is accepted.
    focus(
        &mut e,
        1.2,
        FocusOp::PopScope {
            owner: overlay(1),
            restore: false,
        },
    );
    click(&mut e, 1.3, 20.0, 15.0);
    frame(&mut e, 1.35, modal_frame);
    assert_eq!(e.view().focused(W), Some(&id("bg")));
}

#[test]
fn set_members_refreshes_the_scope() {
    let mut e = engine();
    frame(&mut e, 0.5, modal_frame);
    push(&mut e, 1.0, overlay(1), &[]);
    focus(&mut e, 1.0, FocusOp::Set(id("m1")));
    assert_eq!(e.view().focused(W), None);
    focus(
        &mut e,
        1.1,
        FocusOp::SetMembers {
            owner: overlay(1),
            members: vec![id("m1")],
        },
    );
    focus(&mut e, 1.2, FocusOp::Set(id("m1")));
    assert_eq!(e.view().focused(W), Some(&id("m1")));
    // Dropping the focused member clears focus.
    focus(
        &mut e,
        1.3,
        FocusOp::SetMembers {
            owner: overlay(1),
            members: vec![id("m3")],
        },
    );
    assert_eq!(e.view().focused(W), None);
}

// ---------------------------------------------------------------------------
// Text, IME, clipboard
// ---------------------------------------------------------------------------

#[test]
fn text_keys_and_chars_edit_the_focused_field() {
    let mut e = with_fields();
    focus(&mut e, 1.0, FocusOp::Set(id("a")));
    let fx = typ(&mut e, 1.0, "hello world");
    assert!(!passed(&fx));
    assert_eq!(text(&e, "a"), "hello world");
    assert_eq!(changed_fields(&fx).len(), "hello world".len());

    // Word deletion and movement (Ctrl chords).
    key(&mut e, 1.1, KeyCode::Backspace, ctrl());
    assert_eq!(text(&e, "a"), "hello ");
    key(&mut e, 1.1, KeyCode::ArrowLeft, ctrl());
    typ(&mut e, 1.1, "X");
    assert_eq!(text(&e, "a"), "Xhello ");
    key(&mut e, 1.2, KeyCode::Backspace, no_mods());
    assert_eq!(text(&e, "a"), "hello ");
    key(&mut e, 1.2, KeyCode::End, no_mods());
    key(&mut e, 1.2, KeyCode::ArrowLeft, shift());
    key(&mut e, 1.2, KeyCode::Delete, no_mods());
    assert_eq!(text(&e, "a"), "hello");
    key(&mut e, 1.2, KeyCode::Home, no_mods());
    key(&mut e, 1.2, KeyCode::Delete, ctrl());
    assert_eq!(text(&e, "a"), "");

    // A multi-char key text inserts at once.
    let fx = e.apply(InputOp::Key {
        win: W,
        now: s(1.3),
        input: key_input(KeyCode::A, Some("ab"), no_mods()),
    });
    assert_eq!(text(&e, "a"), "ab");
    assert_eq!(changed_fields(&fx), vec![id("a")]);

    // Enter submits; the text stays.
    let fx = key(&mut e, 1.4, KeyCode::Enter, no_mods());
    assert!(fx.contains(&InputEffect::Text(TextIntent::Submitted {
        win: W,
        field: id("a")
    })));
    assert_eq!(text(&e, "a"), "ab");

    // Not editing chords: they route on.
    for (code, mods) in [
        (KeyCode::Escape, no_mods()),
        (KeyCode::S, ctrl()),
        (KeyCode::ArrowUp, no_mods()),
        (KeyCode::F5, no_mods()),
    ] {
        assert!(passed(&key(&mut e, 1.5, code, mods)), "{code:?}");
    }
    // Key releases always route on.
    let mut release = key_input(KeyCode::A, Some("z"), no_mods());
    release.state = KeyState::Up;
    let fx = e.apply(InputOp::Key {
        win: W,
        now: s(1.5),
        input: release,
    });
    assert!(passed(&fx));
    assert_eq!(text(&e, "a"), "ab");

    // No focused field: text keys route on and change nothing.
    focus(&mut e, 1.6, FocusOp::Clear);
    assert!(passed(&typ(&mut e, 1.6, "q")));
    assert_eq!(text(&e, "a"), "ab");
}

#[test]
fn ime_preedit_stays_out_of_the_text_until_commit() {
    let mut e = with_fields();
    focus(&mut e, 1.0, FocusOp::Set(id("a")));
    typ(&mut e, 1.0, "x");
    let ime = |e: &mut InputEngine, input: ImeInput| {
        e.apply(InputOp::Ime {
            win: W,
            now: s(1.1),
            input,
        })
    };
    ime(&mut e, ImeInput::Enabled);
    let fx = ime(
        &mut e,
        ImeInput::Preedit {
            text: "\u{306b}\u{307b}".into(),
            cursor: Some((3, 3)),
        },
    );
    let state = |e: &InputEngine| {
        e.view()
            .text_fields(W)
            .and_then(|t| t.field_state(&id("a")))
            .map(|s| (s.text.clone(), s.preedit.clone(), s.preedit_cursor))
    };
    assert_eq!(state(&e), Some(("x".into(), "\u{306b}\u{307b}".into(), 1)));
    assert!(changed_fields(&fx).is_empty());
    let fx = ime(&mut e, ImeInput::Commit("\u{65e5}\u{672c}".into()));
    assert_eq!(
        state(&e),
        Some(("x\u{65e5}\u{672c}".into(), String::new(), 0))
    );
    assert_eq!(changed_fields(&fx), vec![id("a")]);
    // Disabling mid-composition drops the preedit, not the text.
    ime(
        &mut e,
        ImeInput::Preedit {
            text: "k".into(),
            cursor: None,
        },
    );
    ime(&mut e, ImeInput::Disabled);
    assert_eq!(
        state(&e),
        Some(("x\u{65e5}\u{672c}".into(), String::new(), 0))
    );
}

#[test]
fn paste_arrives_from_the_clipboard_result() {
    let mut e = with_fields();
    focus(&mut e, 1.0, FocusOp::Set(id("a")));
    typ(&mut e, 1.0, "ab");
    let fx = key(&mut e, 1.1, KeyCode::V, ctrl());
    let ticket = fx.iter().find_map(|f| match f {
        InputEffect::NeedPaste { ticket, .. } => Some(*ticket),
        _ => None,
    });
    assert!(ticket.is_some_and(|t| t.0 >= INPUT_TICKET_BASE));
    assert_eq!(text(&e, "a"), "ab");
    let answer = |t: Option<Ticket>, s: &str| ClipboardResult {
        ticket: t.unwrap_or(Ticket(0)),
        text: Some(s.into()),
    };
    // Someone else's ticket is ignored.
    e.apply(InputOp::Clipboard {
        win: W,
        now: s(1.2),
        result: answer(Some(Ticket(7)), "nope"),
    });
    assert_eq!(text(&e, "a"), "ab");
    let fx = e.apply(InputOp::Clipboard {
        win: W,
        now: s(1.2),
        result: answer(ticket, "xyz"),
    });
    assert_eq!(text(&e, "a"), "abxyz");
    assert_eq!(changed_fields(&fx), vec![id("a")]);
    // Answered once only.
    e.apply(InputOp::Clipboard {
        win: W,
        now: s(1.3),
        result: answer(ticket, "xyz"),
    });
    assert_eq!(text(&e, "a"), "abxyz");

    // Focus moved before the answer: dropped.
    let fx = key(&mut e, 1.4, KeyCode::V, ctrl());
    let second = fx.iter().find_map(|f| match f {
        InputEffect::NeedPaste { ticket, .. } => Some(*ticket),
        _ => None,
    });
    assert_ne!(second, ticket);
    focus(&mut e, 1.5, FocusOp::Set(id("b")));
    e.apply(InputOp::Clipboard {
        win: W,
        now: s(1.6),
        result: answer(second, "late"),
    });
    assert_eq!(text(&e, "a"), "abxyz");
    assert_eq!(text(&e, "b"), "");
}

#[test]
fn copy_and_cut_emit_clipboard_text() {
    let mut e = engine();
    frame(&mut e, 0.5, |c| {
        field(c, "a", rect_a(), TextFieldConfig::text());
        field(c, "pw", rect_b(), TextFieldConfig::password());
    });
    focus(&mut e, 1.0, FocusOp::Set(id("a")));
    typ(&mut e, 1.0, "hello");
    // Nothing selected: nothing copied.
    assert!(copied(&key(&mut e, 1.1, KeyCode::C, ctrl())).is_empty());
    key(&mut e, 1.1, KeyCode::A, ctrl());
    let fx = key(&mut e, 1.2, KeyCode::C, ctrl());
    assert_eq!(copied(&fx), vec!["hello".to_owned()]);
    assert!(changed_fields(&fx).is_empty());
    assert_eq!(text(&e, "a"), "hello");
    let fx = key(&mut e, 1.3, KeyCode::X, ctrl());
    assert_eq!(copied(&fx), vec!["hello".to_owned()]);
    assert_eq!(changed_fields(&fx), vec![id("a")]);
    assert_eq!(text(&e, "a"), "");

    // A masked field never copies or cuts.
    focus(&mut e, 1.4, FocusOp::Set(id("pw")));
    typ(&mut e, 1.4, "secret");
    key(&mut e, 1.4, KeyCode::A, ctrl());
    assert!(copied(&key(&mut e, 1.5, KeyCode::C, ctrl())).is_empty());
    assert!(copied(&key(&mut e, 1.5, KeyCode::X, ctrl())).is_empty());
    assert_eq!(text(&e, "pw"), "secret");
}

#[test]
fn ime_area_and_allowed_are_reported_only_on_change() {
    let mut e = with_fields();
    let areas = |fx: &InputEffects| {
        fx.iter()
            .filter_map(|f| match f {
                InputEffect::ImeArea { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let allowed = |fx: &InputEffects| {
        fx.iter()
            .filter_map(|f| match f {
                InputEffect::ImeAllowed { allowed, .. } => Some(*allowed),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let fx = focus(&mut e, 1.0, FocusOp::Set(id("a")));
    assert_eq!(allowed(&fx), vec![true]);
    assert_eq!(areas(&fx), vec![Rect::new(10.0, 10.0, 1.0, 20.0)]);
    // Same caret: nothing new.
    let fx = focus(&mut e, 1.1, FocusOp::Set(id("a")));
    assert!(areas(&fx).is_empty() && allowed(&fx).is_empty());
    typ(&mut e, 1.2, "ab");
    // Fresh geometry after the next frame moves the caret two chars on.
    let fx = frame(&mut e, 1.3, two_fields);
    assert_eq!(
        areas(&fx),
        vec![Rect::new(10.0 + 2.0 * CHAR_W, 10.0, 1.0, 20.0)]
    );
    let fx = frame(&mut e, 1.4, two_fields);
    assert!(areas(&fx).is_empty());
    let fx = focus(&mut e, 1.5, FocusOp::Clear);
    assert_eq!(allowed(&fx), vec![false]);
}

// ---------------------------------------------------------------------------
// Keymap interplay (kernel key steps 1 and 3)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Ov {
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Act {
    Save,
    CloseSettings,
    SubmitField,
    SelectEverything,
}

fn bind(k: &mut KeymapEngine<Ov, Act>, scope: KeymapScope<Ov>, chord: KeyboardShortcut, a: Act) {
    k.apply(KeymapOp::Cmd(KeymapCmd::Bind {
        scope,
        binding: Binding::new(chord, a),
    }));
}

/// Key steps 1 and 3 as the kernel runs them.
fn route_key(
    e: &mut InputEngine,
    k: &KeymapEngine<Ov, Act>,
    key: KeyInput,
    top: Option<Ov>,
) -> Option<Act> {
    let fx = e.apply(InputOp::Key {
        win: W,
        now: s(2.0),
        input: key,
    });
    let focused = e.view().focused(W).cloned();
    fx.iter().find_map(|f| match f {
        InputEffect::KeyPassed { key, .. } => k.resolve(
            &KeyboardShortcut::new(key.mods, key.code),
            focused.as_ref(),
            top.as_ref(),
            top.is_some(),
        ),
        _ => None,
    })
}

/// H2 §7 scenario 5 with focus and scope coming from the InputEngine.
#[test]
fn keymap_precedence_with_engine_focus_and_scope() {
    let mut e = engine();
    frame(&mut e, 0.5, modal_frame);
    let mut k = KeymapEngine::<Ov, Act>::new();
    let ctrl_s = KeyboardShortcut::new(ctrl(), KeyCode::S);
    bind(&mut k, KeymapScope::Global, ctrl_s.clone(), Act::Save);
    bind(
        &mut k,
        KeymapScope::Overlay(Ov::Settings),
        ctrl_s.clone(),
        Act::CloseSettings,
    );
    bind(
        &mut k,
        KeymapScope::Focused(id("m2")),
        ctrl_s,
        Act::SubmitField,
    );
    let press = || key_input(KeyCode::S, None, ctrl());

    push(&mut e, 1.0, overlay(1), &["m1", "m2"]);
    focus(&mut e, 1.0, FocusOp::Set(id("m2")));
    // A widget inside the modal is focused (a text field: Ctrl+S is not an
    // editing chord, so it reaches the keymap).
    assert_eq!(
        route_key(&mut e, &k, press(), Some(Ov::Settings)),
        Some(Act::SubmitField)
    );
    // The modal is active, nothing inside focused.
    focus(&mut e, 1.1, FocusOp::Clear);
    assert_eq!(
        route_key(&mut e, &k, press(), Some(Ov::Settings)),
        Some(Act::CloseSettings)
    );
    // No overlay: global, the same tick the scope closes.
    focus(
        &mut e,
        1.2,
        FocusOp::PopScope {
            owner: overlay(1),
            restore: true,
        },
    );
    assert_eq!(route_key(&mut e, &k, press(), None), Some(Act::Save));
}

#[test]
fn editing_chords_beat_keymap_bindings_in_a_focused_field() {
    let mut e = with_fields();
    let mut k = KeymapEngine::<Ov, Act>::new();
    let ctrl_a = KeyboardShortcut::new(ctrl(), KeyCode::A);
    bind(
        &mut k,
        KeymapScope::Global,
        ctrl_a.clone(),
        Act::SelectEverything,
    );
    bind(
        &mut k,
        KeymapScope::Focused(id("a")),
        ctrl_a,
        Act::SelectEverything,
    );
    focus(&mut e, 1.0, FocusOp::Set(id("a")));
    typ(&mut e, 1.0, "abc");
    let press = || key_input(KeyCode::A, None, ctrl());
    // The field selects all; no binding resolves.
    assert_eq!(route_key(&mut e, &k, press(), None), None);
    assert_eq!(
        e.view()
            .text_fields(W)
            .and_then(|t| t.selection_range(&id("a"))),
        Some((0, 3))
    );
    // Without a focused field the binding wins.
    focus(&mut e, 1.1, FocusOp::Clear);
    assert_eq!(
        route_key(&mut e, &k, press(), None),
        Some(Act::SelectEverything)
    );
}

// ---------------------------------------------------------------------------
// Caret blink
// ---------------------------------------------------------------------------

/// A focused, engaged field `a` typed into at `t`.
fn engaged_at(t: f64) -> (InputEngine, InputEffects) {
    let mut e = with_fields();
    focus(&mut e, t, FocusOp::Set(id("a")));
    let fx = typ(&mut e, t, "x");
    (e, fx)
}

#[test]
fn an_idle_focused_field_wakes_at_each_blink_edge_and_alternates() {
    let (mut e, fx) = engaged_at(10.0);
    assert_eq!(deadlines(&fx), vec![Some(s(10.5))]);
    assert!(e.view().caret_visible(W));
    assert_eq!(e.view().caret_deadline(W), Some(s(10.5)));

    // Not due yet: nothing happens.
    assert!(e.tick(s(10.2)).is_empty());

    let mut seen = Vec::new();
    for t in [10.5, 11.0, 11.5, 12.0] {
        let rev = e.revision();
        let fx = e.tick(s(t));
        assert_eq!(e.revision(), rev.next());
        // Only the field's own rect is repainted.
        assert_eq!(invalidated(&fx), vec![Some(rect_a())]);
        seen.push((e.view().caret_visible(W), deadlines(&fx)));
    }
    assert_eq!(
        seen,
        vec![
            (false, vec![Some(s(11.0))]),
            (true, vec![Some(s(11.5))]),
            (false, vec![Some(s(12.0))]),
            (true, vec![Some(s(12.5))]),
        ]
    );
    // The store agrees with the published phase.
    assert_eq!(
        e.view().text_fields(W).map(|t| t.cursor_visible(12_100)),
        Some(true)
    );
}

#[test]
fn a_host_waking_late_still_lands_on_the_phase_grid() {
    let (mut e, _) = engaged_at(10.0);
    // The host wakes 120 ms late: the caret is in its hidden phase and the
    // next edge stays on the 500 ms grid anchored at the edit.
    let fx = e.tick(s(10.62));
    assert!(!e.view().caret_visible(W));
    assert_eq!(deadlines(&fx), vec![Some(s(11.0))]);
}

#[test]
fn typing_resets_the_blink_to_visible() {
    let (mut e, _) = engaged_at(10.0);
    e.tick(s(10.5));
    assert!(!e.view().caret_visible(W));
    let fx = typ(&mut e, 10.7, "y");
    assert!(e.view().caret_visible(W));
    assert_eq!(deadlines(&fx), vec![Some(s(11.2))]);
    assert_eq!(invalidated(&fx), vec![Some(rect_a())]);
    // The old edge no longer fires anything.
    assert!(e.tick(s(11.0)).is_empty());
    e.tick(s(11.2));
    assert!(!e.view().caret_visible(W));
    // Caret movement counts as editing too.
    let fx = key(&mut e, 11.3, KeyCode::Home, no_mods());
    assert!(e.view().caret_visible(W));
    assert_eq!(deadlines(&fx), vec![Some(s(11.8))]);
}

#[test]
fn blur_window_unfocus_and_unregister_stop_the_wakes() {
    // Blur.
    let (mut e, _) = engaged_at(10.0);
    let fx = focus(&mut e, 10.2, FocusOp::Clear);
    assert_eq!(deadlines(&fx), vec![None]);
    assert!(!e.view().caret_visible(W));
    assert!(e.tick(s(10.5)).is_empty());
    assert!(e.tick(s(20.0)).is_empty());

    // Window loses OS focus, then regains it.
    let (mut e, _) = engaged_at(10.0);
    let fx = e.apply(InputOp::WindowFocus {
        win: W,
        focused: false,
        now: s(10.2),
    });
    assert_eq!(deadlines(&fx), vec![None]);
    assert!(e.tick(s(10.5)).is_empty());
    let fx = e.apply(InputOp::WindowFocus {
        win: W,
        focused: true,
        now: s(12.1),
    });
    assert_eq!(deadlines(&fx), vec![Some(s(12.5))]);

    // The field is unregistered during compose.
    let (mut e, _) = engaged_at(10.0);
    let fx = frame(&mut e, 10.3, |c| c.unregister(&id("a")));
    assert_eq!(deadlines(&fx), vec![None]);
    assert_eq!(e.view().focused(W), None);
    assert!(e.tick(s(10.5)).is_empty());

    // Merely armed (focused by command, never touched): no blinking.
    let mut e = with_fields();
    let fx = focus(&mut e, 10.0, FocusOp::Set(id("a")));
    assert!(deadlines(&fx).is_empty());
    assert_eq!(e.view().caret_deadline(W), None);
}

#[test]
fn moving_focus_repaints_both_fields() {
    let (mut e, _) = engaged_at(10.0);
    focus(&mut e, 10.1, FocusOp::Set(id("b")));
    // Clicking into b engages it.
    click(&mut e, 10.2, 20.0, 55.0);
    let fx = frame(&mut e, 10.25, two_fields);
    assert_eq!(e.view().focused(W), Some(&id("b")));
    assert!(e.view().caret_visible(W));
    assert_eq!(deadlines(&fx), vec![Some(s(10.75))]);
    let (mut e, _) = engaged_at(10.0);
    let fx = focus(&mut e, 10.1, FocusOp::Set(id("b")));
    assert_eq!(invalidated(&fx), vec![Some(rect_a()), Some(rect_b())]);
}

// ---------------------------------------------------------------------------
// Revision
// ---------------------------------------------------------------------------

#[test]
fn revision_bumps_exactly_on_state_change() {
    let mut e = InputEngine::new();
    assert_eq!(e.revision(), Revision::ZERO);
    let step = |e: &mut InputEngine, op: InputOp, bumps: bool| {
        let before = e.revision();
        e.apply(op);
        let want = if bumps { before.next() } else { before };
        assert_eq!(e.revision(), want);
    };
    step(&mut e, InputOp::Open(W), true);
    step(&mut e, InputOp::Open(W), false);
    step(&mut e, InputOp::Close(WindowId(9)), false);
    let focus_op = |op| InputOp::Focus {
        win: W,
        now: s(1.0),
        op,
    };
    step(
        &mut e,
        InputOp::Pointer {
            win: W,
            now: s(1.0),
            input: moved(1.0, 1.0),
        },
        false,
    );
    step(&mut e, focus_op(FocusOp::Set(id("w"))), true);
    step(&mut e, focus_op(FocusOp::Set(id("w"))), false);
    step(&mut e, focus_op(FocusOp::Clear), true);
    step(&mut e, focus_op(FocusOp::Clear), false);
    let cap = |c| InputOp::SetCapture { win: W, capture: c };
    step(
        &mut e,
        cap(Some(Capture::Engine(EngineTarget::Layout))),
        true,
    );
    step(
        &mut e,
        cap(Some(Capture::Engine(EngineTarget::Layout))),
        false,
    );
    step(&mut e, cap(None), true);
    step(
        &mut e,
        focus_op(FocusOp::PushScope {
            owner: overlay(1),
            members: vec![],
        }),
        true,
    );
    step(&mut e, focus_op(FocusOp::Set(id("outside"))), false);
    step(
        &mut e,
        focus_op(FocusOp::PopScope {
            owner: overlay(7),
            restore: true,
        }),
        false,
    );
    step(
        &mut e,
        focus_op(FocusOp::PopScope {
            owner: overlay(1),
            restore: true,
        }),
        true,
    );
    step(
        &mut e,
        InputOp::Key {
            win: W,
            now: s(1.0),
            input: key_input(KeyCode::S, None, ctrl()),
        },
        false,
    );
    // Frames with nothing registered change nothing.
    step(
        &mut e,
        InputOp::BeginFrame {
            win: W,
            now: s(1.0),
        },
        false,
    );
    step(
        &mut e,
        InputOp::EndFrame {
            win: W,
            now: s(1.0),
        },
        false,
    );
    // Ops on unknown windows change nothing.
    step(
        &mut e,
        InputOp::BeginFrame {
            win: WindowId(9),
            now: s(1.0),
        },
        false,
    );
    step(&mut e, InputOp::Close(W), true);
}

// ---------------------------------------------------------------------------
// T2: plain-text selection owner
// ---------------------------------------------------------------------------

use uzor::input::text::selection::{SelectionLine, TextSelection};
use uzor::input::{LayerId, WidgetKind};

/// The label fixture text; 11 chars, one word at 0..5.
const LABEL: &str = "hello world";

fn label_rect() -> Rect {
    Rect::new(10.0, 10.0, 200.0, 20.0)
}

/// Report geometry for an already-registered selectable widget (one char
/// every `CHAR_W` px from the rect's left edge).
fn report(e: &mut InputEngine, name: &str, rect: Rect, text: &str) {
    let n = text.chars().count();
    let boundaries: Vec<f64> = (0..=n).map(|i| rect.x + i as f64 * CHAR_W).collect();
    let lines = vec![SelectionLine::single(&boundaries, rect.y, rect.height)];
    e.update_selectable(W, id(name), text, lines);
}

/// Register a top-level selectable single-line label and report its geometry.
fn label(e: &mut InputEngine, name: &str, rect: Rect, text: &str) {
    e.registrar(W)
        .unwrap()
        .register(id(name), rect, Sense::HOVER.with_select());
    report(e, name, rect, text);
}

/// Two labels inside a `Panel`, one outside — the Ctrl+A escalation scene.
fn panel_scene(e: &mut InputEngine) {
    let layer = LayerId::from("base");
    let c = e.registrar(W).unwrap();
    c.push_layer(layer.clone(), 0, false);
    let panel = c.register_composite(
        "panel",
        WidgetKind::Panel,
        Rect::new(0.0, 0.0, 400.0, 90.0),
        Sense::NONE,
        &layer,
    );
    c.register_child(
        &panel,
        "in1",
        WidgetKind::Custom,
        rect_a(),
        Sense::HOVER.with_select(),
    );
    c.register_child(
        &panel,
        "in2",
        WidgetKind::Custom,
        rect_b(),
        Sense::HOVER.with_select(),
    );
    drop(c);
    report(e, "in1", rect_a(), LABEL);
    report(e, "in2", rect_b(), "inside two");
    label(e, "out", Rect::new(10.0, 200.0, 200.0, 20.0), "outside");
}

/// A compose frame whose closure sees the whole engine (registrations plus
/// selectable reports).
fn frame_sel(e: &mut InputEngine, t: f64, f: impl FnOnce(&mut InputEngine)) -> InputEffects {
    e.apply(InputOp::BeginFrame { win: W, now: s(t) });
    f(e);
    e.apply(InputOp::EndFrame { win: W, now: s(t) })
}

fn one_label(e: &mut InputEngine) {
    label(e, "lbl", label_rect(), LABEL);
}

fn sel(e: &InputEngine, name: &str) -> Option<TextSelection> {
    e.view()
        .selections(W)
        .and_then(|s| s.iter().find(|(w, _)| w == &id(name)).map(|(_, x)| *x))
}

fn has_copy(effects: &InputEffects, want: &str) -> bool {
    effects.iter().any(
        |f| matches!(f, InputEffect::CopyText { win, text } if *win == W && text.as_str() == want),
    )
}

fn has_passed(effects: &InputEffects) -> bool {
    effects
        .iter()
        .any(|f| matches!(f, InputEffect::KeyPassed { win, .. } if *win == W))
}

#[test]
fn click_collapses_to_caret() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    click(&mut e, 1.0, 13.0, 15.0);
    frame_sel(&mut e, 1.05, one_label);
    // Mid of char 0 is x=14; x=13 is inside char 0.
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::caret(0)));
}

#[test]
fn double_click_selects_word_triple_selects_line() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    // x=20 is inside char 1 ("e"), word 0..5, line 0..11.
    click(&mut e, 1.0, 20.0, 15.0);
    frame_sel(&mut e, 1.05, one_label);
    click(&mut e, 1.1, 20.0, 15.0);
    frame_sel(&mut e, 1.15, one_label);
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::new(0, 5)));
    click(&mut e, 1.2, 20.0, 15.0);
    frame_sel(&mut e, 1.25, one_label);
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::new(0, 11)));
}

#[test]
fn drag_selects_range_and_survives_release() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    to_content(&mut e, 1.0, down(13.0, 15.0));
    // 37 px past the 6 px threshold arms the drag; x=50 is char 5.
    to_content(&mut e, 1.01, moved(50.0, 15.0));
    frame_sel(&mut e, 1.05, one_label);
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::new(0, 5)));
    to_content(&mut e, 1.06, up(50.0, 15.0));
    // The release ended a drag: it must not collapse the selection.
    frame_sel(&mut e, 1.1, one_label);
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::new(0, 5)));
}

#[test]
fn drag_clamps_beyond_the_rect() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    to_content(&mut e, 1.0, down(13.0, 15.0));
    to_content(&mut e, 1.01, moved(900.0, 15.0));
    frame_sel(&mut e, 1.05, one_label);
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::new(0, 11)));
    to_content(&mut e, 1.06, up(900.0, 15.0));
    to_content(&mut e, 1.1, down(60.0, 15.0));
    to_content(&mut e, 1.11, moved(-500.0, 15.0));
    frame_sel(&mut e, 1.15, one_label);
    // Backward drag from char 6 to char 0.
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::new(6, 0)));
}

#[test]
fn press_elsewhere_clears() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    to_content(&mut e, 1.0, down(13.0, 15.0));
    to_content(&mut e, 1.01, moved(50.0, 15.0));
    frame_sel(&mut e, 1.05, one_label);
    assert!(sel(&e, "lbl").is_some());
    // Press on empty space (no widget under the pointer).
    to_content(&mut e, 1.1, down(500.0, 400.0));
    to_content(&mut e, 1.11, up(500.0, 400.0));
    frame_sel(&mut e, 1.15, one_label);
    assert_eq!(sel(&e, "lbl"), None);
}

#[test]
fn ctrl_c_copies_the_selection_reading_order() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, panel_scene);
    // Hover the top label so the escalation has a context, then
    // widget-level and panel-level select-all.
    to_content(&mut e, 0.9, moved(20.0, 15.0));
    frame_sel(&mut e, 0.95, panel_scene);
    key(&mut e, 1.0, KeyCode::A, ctrl());
    key(&mut e, 1.1, KeyCode::A, ctrl());
    assert_eq!(sel(&e, "in1"), Some(TextSelection::new(0, 11)));
    assert_eq!(sel(&e, "in2"), Some(TextSelection::new(0, 10)));
    let fx = key(&mut e, 2.0, KeyCode::C, ctrl());
    assert!(has_copy(&fx, "hello world\ninside two"), "effects: {fx:?}");
}

#[test]
fn ctrl_c_without_selection_passes_the_key() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    let fx = key(&mut e, 1.0, KeyCode::C, ctrl());
    assert!(has_passed(&fx));
    assert!(!has_copy(&fx, ""));
}

#[test]
fn ctrl_a_escalates_widget_panel_window() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, panel_scene);
    // Hover the first label so the first Ctrl+A has a context.
    to_content(&mut e, 0.9, moved(20.0, 15.0));
    frame_sel(&mut e, 0.95, panel_scene);
    let fx = key(&mut e, 1.0, KeyCode::A, ctrl());
    assert_eq!(sel(&e, "in1"), Some(TextSelection::new(0, 11)));
    assert!(fx
        .iter()
        .any(|f| matches!(f, InputEffect::InvalidateField { .. })));
    // Second press: everything inside the panel.
    key(&mut e, 1.1, KeyCode::A, ctrl());
    assert_eq!(sel(&e, "in1"), Some(TextSelection::new(0, 11)));
    assert_eq!(sel(&e, "in2"), Some(TextSelection::new(0, 10)));
    assert_eq!(sel(&e, "out"), None);
    // Third press: the whole window.
    key(&mut e, 1.2, KeyCode::A, ctrl());
    assert_eq!(sel(&e, "out"), Some(TextSelection::new(0, 7)));
    // Fourth press: already everywhere, nothing changes.
    let before = e.revision();
    key(&mut e, 1.3, KeyCode::A, ctrl());
    assert_eq!(e.revision(), before);
}

#[test]
fn a_focused_field_consumes_ctrl_c() {
    let mut e = with_fields();
    frame_sel(&mut e, 0.5, |e| {
        two_fields(e.registrar(W).unwrap());
        one_label(e);
    });
    // Select label text, then focus a field (via focus op, so the label
    // selection survives) with no selection of its own.
    to_content(&mut e, 1.0, down(13.0, 15.0));
    to_content(&mut e, 1.01, moved(50.0, 15.0));
    frame_sel(&mut e, 1.05, |e| {
        two_fields(e.registrar(W).unwrap());
        one_label(e);
    });
    assert!(sel(&e, "lbl").is_some());
    focus(&mut e, 1.1, FocusOp::Set(id("a")));
    frame_sel(&mut e, 1.15, |e| {
        two_fields(e.registrar(W).unwrap());
        one_label(e);
    });
    let fx = key(&mut e, 2.0, KeyCode::C, ctrl());
    // The field consumed the chord: no field selection -> no copy at all,
    // and the label selection is not copied either.
    assert!(!has_copy(&fx, "hello"));
    assert!(!has_passed(&fx));
}

#[test]
fn an_unreported_widget_loses_its_selection() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    to_content(&mut e, 1.0, down(13.0, 15.0));
    to_content(&mut e, 1.01, moved(50.0, 15.0));
    frame_sel(&mut e, 1.05, one_label);
    assert!(sel(&e, "lbl").is_some());
    // Next frame reports nothing: the label is gone.
    frame_sel(&mut e, 1.1, |_| {});
    assert_eq!(e.view().selections(W), Some(&[][..]));
}

#[test]
fn selection_changes_are_revisioned() {
    let mut e = engine();
    frame_sel(&mut e, 0.5, one_label);
    let r0 = e.revision();
    click(&mut e, 1.0, 13.0, 15.0);
    let fx = frame_sel(&mut e, 1.05, one_label);
    let r1 = e.revision();
    assert!(r1 > r0, " InvalidateField effects: {fx:?}");
    // An unrelated empty frame does not bump.
    frame_sel(&mut e, 1.1, one_label);
    assert_eq!(e.revision(), r1);
}

#[test]
fn clicking_a_label_does_not_blur_a_focused_field() {
    // The label sits below the fields — no overlap with rect_a / rect_b.
    let label_r = Rect::new(10.0, 100.0, 200.0, 20.0);
    let scene = |e: &mut InputEngine| {
        two_fields(e.registrar(W).unwrap());
        label(e, "lbl", label_r, LABEL);
    };
    let mut e = with_fields();
    frame_sel(&mut e, 0.5, scene);
    click(&mut e, 1.0, 20.0, 15.0); // field "a"
    frame_sel(&mut e, 1.05, scene);
    assert!(e
        .view()
        .text_fields(W)
        .is_some_and(|t| t.focused().is_some()));
    // Click the label: focus stays in the field, the label gets a caret.
    click(&mut e, 1.1, 13.0, 115.0);
    frame_sel(&mut e, 1.15, scene);
    assert!(e
        .view()
        .text_fields(W)
        .is_some_and(|t| t.focused().is_some()));
    assert_eq!(sel(&e, "lbl"), Some(TextSelection::caret(0)));
}
