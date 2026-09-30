//! OverlayEngine tests, one per policy rule (design §3.3 table, §9.2
//! "Modal shield", "Outside dismiss", "Escape order"). Routing is driven the
//! way the kernel will drive it: `Intercept` after cook, `Click` in phase 6,
//! and the key verdict feeds `KeymapEngine::resolve`.

use super::*;
use crate::engine::input::InputEngine;
use crate::engine::keymap::KeymapEngine;
use crate::types::bus::{WheelDelta, WheelInput};
use crate::types::command::{Binding, KeymapCmd, KeymapScope};
use crate::types::ops::{FocusOp, InputOp, KeymapOp};
use uzor::input::{KeyboardShortcut, ModifierKeys, MouseButton};

const W: WindowId = WindowId(1);
const W2: WindowId = WindowId(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Ov {
    Settings,
    Confirm,
    Menu,
    Ctx,
    Tip,
    Panel,
    Picker,
}

type E = OverlayEngine<Ov>;
type Fx = OverlayEffects<Ov>;

fn t(s: f64) -> Seconds {
    Seconds(s)
}

fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x, y, w, h)
}

fn fixed(w: f64, h: f64) -> OverlaySize {
    OverlaySize::Fixed {
        width: w,
        height: h,
    }
}

fn viewport() -> Rect {
    r(0.0, 0.0, 800.0, 600.0)
}

/// An engine whose window `W` has an 800x600 viewport.
fn engine() -> E {
    let mut e = E::new();
    e.apply(OverlayOp::Reclamp {
        win: W,
        viewport: viewport(),
    });
    e
}

fn open_with(
    e: &mut E,
    id: Ov,
    kind: OverlayKind,
    anchor: Option<Rect>,
    size: OverlaySize,
    policy: Option<OverlayPolicy>,
) -> Fx {
    e.apply(OverlayOp::Cmd {
        cmd: OverlayCmd::Open {
            win: W,
            id,
            kind,
            anchor,
            size,
            policy,
        },
        now: t(1.0),
    })
}

/// Open `id` of `kind` at the rect `at` (anchored just above it).
fn open_at(e: &mut E, id: Ov, kind: OverlayKind, at: Rect) -> Fx {
    let anchor = r(at.x, at.y, 0.0, 0.0);
    open_with(e, id, kind, Some(anchor), fixed(at.width, at.height), None)
}

fn modal(e: &mut E, id: Ov, at: Rect) -> Fx {
    open_at(e, id, OverlayKind::Modal, at)
}

fn no_mods() -> ModifierKeys {
    ModifierKeys::default()
}

fn down(x: f64, y: f64) -> Intercepted {
    Intercepted::Pointer(PointerInput::Down {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: no_mods(),
    })
}

fn up(x: f64, y: f64) -> Intercepted {
    Intercepted::Pointer(PointerInput::Up {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: no_mods(),
    })
}

fn moved(x: f64, y: f64) -> Intercepted {
    Intercepted::Pointer(PointerInput::Moved {
        pos: Point::new(x, y),
        mods: no_mods(),
    })
}

fn key(code: KeyCode, state: KeyState) -> Intercepted {
    Intercepted::Key(KeyInput {
        code,
        text: None,
        state,
        mods: no_mods(),
    })
}

fn esc() -> Intercepted {
    key(KeyCode::Escape, KeyState::Down)
}

fn intercept(e: &mut E, event: Intercepted) -> Fx {
    e.apply(OverlayOp::Intercept { win: W, event })
}

/// The single pointer verdict of an intercept (asserts there is exactly one).
fn proute(fx: &Fx) -> PointerRoute<Ov> {
    let routes: Vec<_> = fx
        .iter()
        .filter_map(|f| match f {
            OverlayEffect::Pointer { route, .. } => Some(*route),
            _ => None,
        })
        .collect();
    assert_eq!(routes.len(), 1, "exactly one pointer verdict: {fx:?}");
    assert!(
        matches!(fx.last(), Some(OverlayEffect::Pointer { .. })),
        "the verdict comes last"
    );
    routes[0]
}

fn kroute(fx: &Fx) -> KeyRoute<Ov> {
    let routes: Vec<_> = fx
        .iter()
        .filter_map(|f| match f {
            OverlayEffect::Key { route, .. } => Some(*route),
            _ => None,
        })
        .collect();
    assert_eq!(routes.len(), 1, "exactly one key verdict: {fx:?}");
    routes[0]
}

/// `(id, cause)` of every close, in report order.
fn closes(fx: &Fx) -> Vec<(Ov, CloseCause)> {
    fx.iter()
        .filter_map(|f| match f {
            OverlayEffect::Closed { id, cause, .. } => Some((*id, *cause)),
            _ => None,
        })
        .collect()
}

fn ids(e: &E) -> Vec<Ov> {
    e.view().stack(W).map(|v| v.id()).collect()
}

fn slot_of(e: &E, id: Ov) -> OverlaySlot {
    e.view()
        .get(W, id)
        .map(|v| v.slot())
        .unwrap_or(OverlaySlot(0))
}

fn host(e: &E, id: Ov) -> String {
    e.view()
        .get(W, id)
        .map(|v| v.host_id().as_str().to_owned())
        .unwrap_or_default()
}

fn click(e: &mut E, widget: &str) -> Fx {
    e.apply(OverlayOp::Click {
        win: W,
        widget: WidgetId::from(widget),
        cursor: Point::new(0.0, 0.0),
    })
}

// ---------------------------------------------------------------------------
// Modal shield
// ---------------------------------------------------------------------------

#[test]
fn modal_shield_consumes_pointer_outside_top_modal() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        r(0.0, 0.0, 300.0, 300.0),
    );
    modal(&mut e, Ov::Settings, r(200.0, 150.0, 400.0, 300.0));

    // Outside the modal, over nothing: consumed, no hit below.
    for ev in [down(700.0, 50.0), up(700.0, 50.0), moved(700.0, 50.0)] {
        assert_eq!(proute(&intercept(&mut e, ev)), PointerRoute::Consumed);
    }
    // Outside the modal but over the lower popup: still consumed, and the
    // hit reports the shield, not the popup.
    assert_eq!(
        proute(&intercept(&mut e, down(50.0, 50.0))),
        PointerRoute::Consumed
    );
    assert_eq!(
        e.hit(W, Point::new(50.0, 50.0)),
        OverlayHit::Shielded {
            id: Ov::Settings,
            slot: slot_of(&e, Ov::Settings)
        }
    );
    assert!(e.topmost_at(W, Point::new(50.0, 50.0)).is_none());
    // Wheel outside: consumed too.
    let wheel = Intercepted::Wheel(WheelInput {
        pos: Point::new(700.0, 500.0),
        delta: WheelDelta::Lines { x: 0.0, y: 1.0 },
        mods: no_mods(),
    });
    assert_eq!(proute(&intercept(&mut e, wheel)), PointerRoute::Consumed);
    // Inside the modal: it takes the event.
    let inside = proute(&intercept(&mut e, down(300.0, 200.0)));
    assert_eq!(
        inside,
        PointerRoute::Overlay {
            id: Ov::Settings,
            slot: slot_of(&e, Ov::Settings)
        }
    );
    // The shield never closes anything (default modal: no outside dismiss).
    assert_eq!(ids(&e), vec![Ov::Panel, Ov::Settings]);
}

#[test]
fn modal_shield_key_verdict_blocks_globals_unless_through_modal() {
    let mut e = engine();
    let mut km: KeymapEngine<Ov, u8> = KeymapEngine::new();
    let key_k = KeyboardShortcut::key(KeyCode::K);
    let key_q = KeyboardShortcut::key(KeyCode::Q);
    for (chord, action, through_modal) in [(key_k.clone(), 1, false), (key_q.clone(), 2, true)] {
        km.apply(KeymapOp::Cmd(KeymapCmd::Bind {
            scope: KeymapScope::Global,
            binding: Binding {
                chord,
                action,
                through_modal,
            },
        }));
    }
    let resolve = |e: &mut E, km: &KeymapEngine<Ov, u8>, chord: &KeyboardShortcut| match kroute(
        &intercept(e, key(chord.key, KeyState::Down)),
    ) {
        KeyRoute::Pass {
            top_overlay,
            modal_open,
        } => km.resolve(chord, None, top_overlay.as_ref(), modal_open),
        KeyRoute::Consumed => None,
    };
    assert_eq!(
        resolve(&mut e, &km, &key_k),
        Some(1),
        "no modal: global resolves"
    );
    modal(&mut e, Ov::Settings, r(100.0, 100.0, 200.0, 200.0));
    assert_eq!(
        e.view().key_context(W),
        KeyRoute::Pass {
            top_overlay: Some(Ov::Settings),
            modal_open: true
        }
    );
    assert_eq!(
        resolve(&mut e, &km, &key_k),
        None,
        "modal blocks the global"
    );
    assert_eq!(
        resolve(&mut e, &km, &key_q),
        Some(2),
        "through_modal passes"
    );
    // Escape goes to the top overlay first.
    let fx = intercept(&mut e, esc());
    assert_eq!(closes(&fx), vec![(Ov::Settings, CloseCause::Escape)]);
    assert_eq!(kroute(&fx), KeyRoute::Consumed);
    assert_eq!(
        resolve(&mut e, &km, &key_k),
        Some(1),
        "global is back the same tick"
    );
}

// ---------------------------------------------------------------------------
// Outside dismiss
// ---------------------------------------------------------------------------

#[test]
fn outside_dismiss_closes_top_on_release_and_consumes() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(10.0, 30.0, 100.0, 200.0),
    );
    let fx = intercept(&mut e, down(500.0, 500.0));
    assert_eq!(proute(&fx), PointerRoute::Consumed, "the press is consumed");
    assert!(closes(&fx).is_empty(), "nothing closes on the press");
    assert_eq!(ids(&e), vec![Ov::Menu]);
    let rev = e.revision();
    let fx = intercept(&mut e, up(500.0, 500.0));
    assert_eq!(closes(&fx), vec![(Ov::Menu, CloseCause::Outside)]);
    assert_eq!(
        proute(&fx),
        PointerRoute::Consumed,
        "the release is not a click"
    );
    assert!(ids(&e).is_empty());
    assert_eq!(e.revision(), rev.next());
    // With nothing open the same click passes.
    intercept(&mut e, down(500.0, 500.0));
    assert_eq!(
        proute(&intercept(&mut e, up(500.0, 500.0))),
        PointerRoute::Pass
    );
}

#[test]
fn outside_dismiss_only_the_top_overlay_is_considered() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 100.0, 100.0),
    );
    open_at(
        &mut e,
        Ov::Ctx,
        OverlayKind::ContextMenu,
        r(300.0, 300.0, 100.0, 100.0),
    );
    // Click inside the lower dropdown, outside the top context menu: only
    // the context menu closes; the dropdown does not get the click.
    intercept(&mut e, down(50.0, 50.0));
    let fx = intercept(&mut e, up(50.0, 50.0));
    assert_eq!(closes(&fx), vec![(Ov::Ctx, CloseCause::Outside)]);
    assert_eq!(proute(&fx), PointerRoute::Consumed);
    assert_eq!(ids(&e), vec![Ov::Menu]);
    // A top overlay without dismiss_on_outside lets the outside press fall
    // through to what is under it (here: the free window).
    let keep = OverlayPolicy {
        dismiss_on_outside: false,
        ..OverlayPolicy::for_kind(OverlayKind::Popup)
    };
    open_with(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        Some(r(500.0, 0.0, 0.0, 0.0)),
        fixed(100.0, 100.0),
        Some(keep),
    );
    assert_eq!(
        proute(&intercept(&mut e, down(300.0, 500.0))),
        PointerRoute::Pass
    );
    let fx = intercept(&mut e, up(300.0, 500.0));
    assert!(closes(&fx).is_empty(), "the dropdown below is not the top");
    assert_eq!(ids(&e), vec![Ov::Menu, Ov::Panel]);
}

#[test]
fn outside_dismiss_press_origin_rule() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(10.0, 10.0, 100.0, 100.0),
    );
    let slot = slot_of(&e, Ov::Menu);
    // Press inside, release outside: no dismiss; the release goes to the
    // overlay that took the press.
    let fx = intercept(&mut e, down(50.0, 50.0));
    assert_eq!(proute(&fx), PointerRoute::Overlay { id: Ov::Menu, slot });
    let fx = intercept(&mut e, up(500.0, 500.0));
    assert!(closes(&fx).is_empty());
    assert_eq!(proute(&fx), PointerRoute::Overlay { id: Ov::Menu, slot });
    // Press outside, release inside: no dismiss either; still consumed.
    intercept(&mut e, down(500.0, 500.0));
    let fx = intercept(&mut e, up(50.0, 50.0));
    assert!(closes(&fx).is_empty());
    assert_eq!(proute(&fx), PointerRoute::Consumed);
    // Press outside, OS cancels, release outside: no dismiss.
    intercept(&mut e, down(500.0, 500.0));
    intercept(&mut e, Intercepted::Pointer(PointerInput::Cancelled));
    let fx = intercept(&mut e, up(500.0, 500.0));
    assert!(closes(&fx).is_empty());
    assert_eq!(ids(&e), vec![Ov::Menu]);
    // A right press outside dismisses like a left one.
    let right = |down: bool| {
        let pos = Point::new(600.0, 10.0);
        Intercepted::Pointer(if down {
            PointerInput::Down {
                pos,
                button: MouseButton::Right,
                mods: no_mods(),
            }
        } else {
            PointerInput::Up {
                pos,
                button: MouseButton::Right,
                mods: no_mods(),
            }
        })
    };
    intercept(&mut e, right(true));
    assert_eq!(
        closes(&intercept(&mut e, right(false))),
        vec![(Ov::Menu, CloseCause::Outside)]
    );
}

// ---------------------------------------------------------------------------
// Escape order
// ---------------------------------------------------------------------------

#[test]
fn escape_order_top_first_then_passes_on() {
    let mut e = engine();
    modal(&mut e, Ov::Settings, r(100.0, 100.0, 400.0, 300.0));
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(120.0, 140.0, 100.0, 100.0),
    );
    assert_eq!(
        ids(&e),
        vec![Ov::Settings, Ov::Menu],
        "dropdown over its modal"
    );

    let fx = intercept(&mut e, esc());
    assert_eq!(closes(&fx), vec![(Ov::Menu, CloseCause::Escape)]);
    assert_eq!(kroute(&fx), KeyRoute::Consumed);
    let fx = intercept(&mut e, esc());
    assert_eq!(closes(&fx), vec![(Ov::Settings, CloseCause::Escape)]);
    let rev = e.revision();
    let fx = intercept(&mut e, esc());
    assert!(closes(&fx).is_empty());
    assert_eq!(
        kroute(&fx),
        KeyRoute::Pass {
            top_overlay: None,
            modal_open: false
        },
        "third Esc reaches the keymap"
    );
    assert_eq!(e.revision(), rev, "a passed key changes nothing");
}

#[test]
fn escape_order_modal_without_escape_stops_the_walk() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 50.0, 50.0),
    );
    let sticky = OverlayPolicy {
        dismiss_on_escape: false,
        ..OverlayPolicy::for_kind(OverlayKind::Modal)
    };
    open_with(
        &mut e,
        Ov::Confirm,
        OverlayKind::Modal,
        None,
        fixed(200.0, 100.0),
        Some(sticky),
    );
    let fx = intercept(&mut e, esc());
    assert!(
        closes(&fx).is_empty(),
        "the dropdown under the modal is out of reach"
    );
    assert_eq!(
        kroute(&fx),
        KeyRoute::Pass {
            top_overlay: Some(Ov::Confirm),
            modal_open: true
        }
    );
    // Only a press closes: repeat and release pass.
    open_at(
        &mut e,
        Ov::Ctx,
        OverlayKind::ContextMenu,
        r(300.0, 250.0, 50.0, 50.0),
    );
    for st in [KeyState::Repeat, KeyState::Up] {
        assert!(closes(&intercept(&mut e, key(KeyCode::Escape, st))).is_empty());
    }
    // A tooltip on top is transparent: Esc reaches the context menu.
    open_at(
        &mut e,
        Ov::Tip,
        OverlayKind::Tooltip,
        r(0.0, 0.0, 10.0, 10.0),
    );
    assert_eq!(
        closes(&intercept(&mut e, esc())),
        vec![(Ov::Ctx, CloseCause::Escape)]
    );
    // Other keys never close.
    assert!(closes(&intercept(&mut e, key(KeyCode::Enter, KeyState::Down))).is_empty());
}

// ---------------------------------------------------------------------------
// Z order
// ---------------------------------------------------------------------------

#[test]
fn z_order_kinds_by_table_then_insertion() {
    let mut e = engine();
    let at = r(0.0, 0.0, 100.0, 100.0);
    open_at(&mut e, Ov::Tip, OverlayKind::Tooltip, at);
    open_at(&mut e, Ov::Ctx, OverlayKind::ContextMenu, at);
    open_at(&mut e, Ov::Menu, OverlayKind::Dropdown, at);
    open_at(&mut e, Ov::Panel, OverlayKind::Popup, at);
    open_at(&mut e, Ov::Picker, OverlayKind::ColorPicker, at);
    // Default table: dropdown 2 < popup 4 < context menu 5 < picker 6 < tooltip 7.
    assert_eq!(
        ids(&e),
        vec![Ov::Menu, Ov::Panel, Ov::Ctx, Ov::Picker, Ov::Tip]
    );
    let zs: Vec<i32> = e.view().stack(W).map(|v| v.z()).collect();
    assert!(zs.windows(2).all(|p| p[0] <= p[1]));
    // The topmost that takes the point: the picker (tooltips are transparent).
    assert_eq!(
        e.topmost_at(W, Point::new(50.0, 50.0)).map(|v| v.id()),
        Some(Ov::Picker)
    );
    assert_eq!(e.view().top(W).map(|v| v.id()), Some(Ov::Picker));

    // Equal kinds: insertion order; re-open counts as a new insertion.
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 100.0, 100.0),
    );
    open_at(
        &mut e,
        Ov::Ctx,
        OverlayKind::Dropdown,
        r(50.0, 50.0, 100.0, 100.0),
    );
    assert_eq!(ids(&e), vec![Ov::Menu, Ov::Ctx]);
    assert_eq!(
        e.topmost_at(W, Point::new(75.0, 75.0)).map(|v| v.id()),
        Some(Ov::Ctx)
    );
    assert_eq!(
        e.topmost_at(W, Point::new(25.0, 25.0)).map(|v| v.id()),
        Some(Ov::Menu)
    );
    assert!(e.topmost_at(W, Point::new(400.0, 400.0)).is_none());
    assert_eq!(e.hit(W, Point::new(400.0, 400.0)), OverlayHit::None);
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 100.0, 100.0),
    );
    assert_eq!(ids(&e), vec![Ov::Ctx, Ov::Menu]);
    assert_eq!(
        e.topmost_at(W, Point::new(75.0, 75.0)).map(|v| v.id()),
        Some(Ov::Menu)
    );
}

#[test]
fn z_order_tooltip_above_modal_and_modal_bands() {
    let mut e = engine();
    // Opened before the modal: below it (and shielded), even though the
    // popup's table z (4) is above the modal's (3).
    open_at(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        r(0.0, 0.0, 800.0, 600.0),
    );
    modal(&mut e, Ov::Settings, r(100.0, 100.0, 300.0, 300.0));
    // Opened by the modal: above it, even the dropdown (table z 2).
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(350.0, 350.0, 200.0, 100.0),
    );
    open_at(
        &mut e,
        Ov::Tip,
        OverlayKind::Tooltip,
        r(110.0, 110.0, 50.0, 20.0),
    );
    assert_eq!(ids(&e), vec![Ov::Panel, Ov::Settings, Ov::Menu, Ov::Tip]);
    let bands: Vec<u32> = e.view().stack(W).map(|v| v.band()).collect();
    assert_eq!(bands, vec![0, 1, 1, 1]);
    // The dropdown sticks out of the modal and still takes its own rect.
    assert_eq!(
        e.topmost_at(W, Point::new(500.0, 420.0)).map(|v| v.id()),
        Some(Ov::Menu)
    );
    // The tooltip is drawn on top but the modal takes the point under it.
    assert_eq!(
        e.topmost_at(W, Point::new(120.0, 115.0)).map(|v| v.id()),
        Some(Ov::Settings)
    );
    // The full-window popup below the modal is never reached.
    assert!(matches!(
        e.hit(W, Point::new(700.0, 50.0)),
        OverlayHit::Shielded { .. }
    ));
    // A second modal opens a new band above everything.
    modal(&mut e, Ov::Confirm, r(200.0, 200.0, 100.0, 100.0));
    assert_eq!(e.view().top(W).map(|v| v.id()), Some(Ov::Confirm));
    assert_eq!(e.view().get(W, Ov::Confirm).map(|v| v.band()), Some(2));
    assert!(matches!(
        e.hit(W, Point::new(500.0, 420.0)),
        OverlayHit::Shielded {
            id: Ov::Confirm,
            ..
        }
    ));
    // Windows are independent.
    assert_eq!(e.hit(W2, Point::new(500.0, 420.0)), OverlayHit::None);
}

// ---------------------------------------------------------------------------
// Re-open, reclamp, auto-close, nested close order
// ---------------------------------------------------------------------------

#[test]
fn reopen_replaces_existing_instance() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 100.0, 100.0),
    );
    let first = slot_of(&e, Ov::Menu);
    let fx = open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(200.0, 0.0, 100.0, 100.0),
    );
    assert_eq!(closes(&fx), vec![(Ov::Menu, CloseCause::Replaced)]);
    assert!(matches!(
        fx.as_slice(),
        [OverlayEffect::Closed { slot: a, .. }, OverlayEffect::Opened { slot: b, .. }]
            if *a == first && *b != first
    ));
    assert_eq!(ids(&e), vec![Ov::Menu]);
    assert_eq!(e.view().get(W, Ov::Menu).map(|v| v.rect().x), Some(200.0));
    // Toggle closes an open one and opens a closed one.
    let toggle = OverlayCmd::Toggle {
        win: W,
        id: Ov::Menu,
        kind: OverlayKind::Dropdown,
        anchor: None,
        size: fixed(10.0, 10.0),
        policy: None,
    };
    let fx = e.apply(OverlayOp::Cmd {
        cmd: toggle,
        now: t(2.0),
    });
    assert_eq!(closes(&fx), vec![(Ov::Menu, CloseCause::Command)]);
    let fx = e.apply(OverlayOp::Cmd {
        cmd: toggle,
        now: t(3.0),
    });
    assert!(matches!(
        fx.as_slice(),
        [OverlayEffect::Opened { id: Ov::Menu, .. }]
    ));
}

#[test]
fn reclamp_keeps_rects_inside_shrunk_viewport() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(600.0, 400.0, 150.0, 150.0),
    );
    modal(&mut e, Ov::Settings, r(0.0, 0.0, 700.0, 500.0));
    let rev = e.revision();
    let small = r(0.0, 0.0, 400.0, 300.0);
    let fx = e.apply(OverlayOp::Reclamp {
        win: W,
        viewport: small,
    });
    assert!(fx.iter().any(|f| matches!(
        f,
        OverlayEffect::Invalidate { bits, .. } if *bits == InvalidateBits::GEOMETRY
    )));
    assert_eq!(e.revision(), rev.next());
    for v in e.view().stack(W) {
        let rc = v.rect();
        assert!(rc.x >= 0.0 && rc.y >= 0.0, "{rc:?}");
        assert!(
            rc.x + rc.width <= 400.0 && rc.y + rc.height <= 300.0,
            "{rc:?}"
        );
    }
    assert_eq!(
        e.view().get(W, Ov::Menu).map(|v| v.rect()),
        Some(r(250.0, 150.0, 150.0, 150.0))
    );
    // Oversized: truncated to the viewport.
    assert_eq!(e.view().get(W, Ov::Settings).map(|v| v.rect()), Some(small));
    // The body state follows the rect.
    assert!(matches!(
        e.view().get(W, Ov::Menu).map(|v| v.body()),
        Some(OverlayBody::Dropdown(d)) if d.effective_origin() == (250.0, 150.0)
    ));
    // Same viewport again: no change, no revision.
    let fx = e.apply(OverlayOp::Reclamp {
        win: W,
        viewport: small,
    });
    assert!(fx.is_empty());
    assert_eq!(e.revision(), rev.next());
    // Opens and moves clamp to the stored viewport.
    open_at(
        &mut e,
        Ov::Ctx,
        OverlayKind::ContextMenu,
        r(390.0, 290.0, 100.0, 100.0),
    );
    assert_eq!(
        e.view().get(W, Ov::Ctx).map(|v| v.rect()),
        Some(r(300.0, 200.0, 100.0, 100.0))
    );
    e.apply(OverlayOp::Cmd {
        cmd: OverlayCmd::Move {
            win: W,
            id: Ov::Ctx,
            to: Point::new(-50.0, 10.0),
        },
        now: t(2.0),
    });
    assert_eq!(
        e.view().get(W, Ov::Ctx).map(|v| v.rect()),
        Some(r(0.0, 10.0, 100.0, 100.0))
    );
}

#[test]
fn auto_close_deadline_fires() {
    let mut e = engine();
    let policy = OverlayPolicy {
        auto_close_after: Some(Seconds(2.5)),
        ..OverlayPolicy::for_kind(OverlayKind::Popup)
    };
    let fx = open_with(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        None,
        fixed(100.0, 50.0),
        Some(policy),
    );
    let slot = slot_of(&e, Ov::Panel);
    assert!(matches!(
        fx.as_slice(),
        [OverlayEffect::Opened { auto_close_at: Some(at), .. }] if *at == Seconds(3.5)
    ));
    let fx = e.apply(OverlayOp::Fire { win: W, slot });
    assert_eq!(closes(&fx), vec![(Ov::Panel, CloseCause::Timer)]);
    assert!(matches!(
        fx.as_slice(),
        [OverlayEffect::Closed {
            auto_close: true,
            ..
        }]
    ));
    // A stale fire (already closed / replaced instance) is a no-op.
    let rev = e.revision();
    assert!(e.apply(OverlayOp::Fire { win: W, slot }).is_empty());
    assert_eq!(e.revision(), rev);
    // Re-open under the same id: the old slot's deadline cannot close it.
    open_with(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        None,
        fixed(1.0, 1.0),
        Some(policy),
    );
    open_with(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        None,
        fixed(1.0, 1.0),
        Some(policy),
    );
    let old = OverlaySlot(slot.0 + 1);
    assert!(e.apply(OverlayOp::Fire { win: W, slot: old }).is_empty());
    assert_eq!(ids(&e), vec![Ov::Panel]);
}

#[test]
fn nested_modal_over_dropdown_over_panel_closes_in_order() {
    // A dropdown opened from a docked panel, then a modal over it.
    let mut e = engine();
    let o1 = open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(10.0, 40.0, 100.0, 100.0),
    );
    let o2 = modal(&mut e, Ov::Settings, r(200.0, 100.0, 300.0, 300.0));
    let opened: Vec<(Ov, Option<ScopeOwner>, bool)> = [o1, o2]
        .iter()
        .flat_map(|fx| fx.iter())
        .filter_map(|f| match f {
            OverlayEffect::Opened {
                id, scope, modal, ..
            } => Some((*id, scope.clone(), *modal)),
            _ => None,
        })
        .collect();
    assert_eq!(
        opened,
        vec![
            (
                Ov::Menu,
                Some(ScopeOwner::Overlay(slot_of(&e, Ov::Menu))),
                false
            ),
            (
                Ov::Settings,
                Some(ScopeOwner::Overlay(slot_of(&e, Ov::Settings))),
                true
            ),
        ]
    );
    assert_eq!(ids(&e), vec![Ov::Menu, Ov::Settings]);
    // The dropdown is under the modal's shield until the modal closes.
    assert!(matches!(
        e.hit(W, Point::new(50.0, 80.0)),
        OverlayHit::Shielded { .. }
    ));
    // Esc x2: modal, then dropdown — each close reports the scope that
    // opened with it and its restore flag, in reverse open order.
    let mut popped = Vec::new();
    for _ in 0..2 {
        for f in intercept(&mut e, esc()) {
            if let OverlayEffect::Closed {
                id,
                scope,
                restore_focus,
                ..
            } = f
            {
                popped.push((id, scope, restore_focus));
            }
        }
    }
    let scopes: Vec<Option<ScopeOwner>> = opened.iter().rev().map(|o| o.1.clone()).collect();
    assert_eq!(
        popped.iter().map(|p| p.0).collect::<Vec<_>>(),
        vec![Ov::Settings, Ov::Menu]
    );
    assert_eq!(
        popped.iter().map(|p| p.1.clone()).collect::<Vec<_>>(),
        scopes
    );
    assert!(popped.iter().all(|p| p.2), "default policies restore focus");

    // Closing a modal by command first closes what it opened, newest first
    // (reverse open order, not z order: the dropdown was opened last but
    // sits below the context menu).
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        r(0.0, 0.0, 50.0, 50.0),
    );
    modal(&mut e, Ov::Settings, r(100.0, 100.0, 300.0, 300.0));
    open_at(
        &mut e,
        Ov::Ctx,
        OverlayKind::ContextMenu,
        r(120.0, 120.0, 50.0, 50.0),
    );
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(110.0, 110.0, 50.0, 50.0),
    );
    assert_eq!(ids(&e), vec![Ov::Panel, Ov::Settings, Ov::Menu, Ov::Ctx]);
    let fx = e.apply(OverlayOp::Cmd {
        cmd: OverlayCmd::Close {
            win: W,
            id: Ov::Settings,
        },
        now: t(2.0),
    });
    assert_eq!(
        closes(&fx),
        vec![
            (Ov::Menu, CloseCause::Command),
            (Ov::Ctx, CloseCause::Command),
            (Ov::Settings, CloseCause::Command)
        ]
    );
    assert_eq!(ids(&e), vec![Ov::Panel]);
}

/// The F7 conduction rows `Opened -> PushScope`, `Closed -> PopScope {
/// restore }` applied to a real InputEngine: focus comes back to the field
/// focused before the first overlay, through every nesting level.
#[test]
fn nested_close_order_restores_focus_through_input_engine() {
    let mut e = engine();
    let mut input = InputEngine::new();
    input.apply(InputOp::Open(W));
    let field = WidgetId::from("panel:field");
    let in_menu = WidgetId::from("menu:field");
    let in_modal = WidgetId::from("modal:field");
    let conduct = |input: &mut InputEngine, fx: &Fx, members: &[WidgetId]| {
        for f in fx {
            let op = match f {
                OverlayEffect::Opened {
                    scope: Some(owner), ..
                } => FocusOp::PushScope {
                    owner: owner.clone(),
                    members: members.to_vec(),
                },
                OverlayEffect::Closed {
                    scope: Some(owner),
                    restore_focus,
                    ..
                } => FocusOp::PopScope {
                    owner: owner.clone(),
                    restore: *restore_focus,
                },
                _ => continue,
            };
            input.apply(InputOp::Focus {
                win: W,
                now: t(1.0),
                op,
            });
        }
    };
    let focus = |input: &mut InputEngine, id: &WidgetId| {
        input.apply(InputOp::Focus {
            win: W,
            now: t(1.0),
            op: FocusOp::Set(id.clone()),
        });
    };
    let focused = |input: &InputEngine| input.view().focused(W).cloned();

    focus(&mut input, &field);
    assert_eq!(focused(&input), Some(field.clone()));
    let fx = open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 100.0, 100.0),
    );
    conduct(&mut input, &fx, std::slice::from_ref(&in_menu));
    focus(&mut input, &in_menu);
    assert_eq!(focused(&input), Some(in_menu.clone()));
    let fx = modal(&mut e, Ov::Settings, r(200.0, 100.0, 300.0, 300.0));
    conduct(&mut input, &fx, std::slice::from_ref(&in_modal));
    focus(&mut input, &field);
    assert_eq!(
        focused(&input),
        None,
        "focus cannot leave the modal's scope"
    );
    focus(&mut input, &in_modal);

    let fx = intercept(&mut e, esc());
    conduct(&mut input, &fx, &[]);
    assert_eq!(
        focused(&input),
        Some(in_menu.clone()),
        "modal closed: back into the menu"
    );
    let fx = intercept(&mut e, esc());
    conduct(&mut input, &fx, &[]);
    assert_eq!(
        focused(&input),
        Some(field),
        "menu closed: back to the panel field"
    );
}

// ---------------------------------------------------------------------------
// Bodies: clicks through the dispatcher and the library consume_event
// ---------------------------------------------------------------------------

#[test]
fn composite_clicks_become_intents_and_item_closes() {
    let mut e = engine();
    modal(&mut e, Ov::Settings, r(100.0, 100.0, 400.0, 300.0));
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(120.0, 140.0, 100.0, 100.0),
    );
    open_at(
        &mut e,
        Ov::Ctx,
        OverlayKind::ContextMenu,
        r(300.0, 300.0, 100.0, 100.0),
    );
    let (hm, hd, hc) = (
        host(&e, Ov::Settings),
        host(&e, Ov::Menu),
        host(&e, Ov::Ctx),
    );

    // Not an overlay widget: nothing.
    let rev = e.revision();
    assert!(click(&mut e, "panel:button").is_empty());
    assert_eq!(e.revision(), rev);

    let fx = click(&mut e, &format!("{hc}:item:2"));
    assert_eq!(
        fx.first(),
        Some(&OverlayEffect::Intent(OverlayIntent::ContextMenuItem {
            win: W,
            id: Ov::Ctx,
            index: 2
        }))
    );
    assert_eq!(closes(&fx), vec![(Ov::Ctx, CloseCause::Item)]);

    let fx = click(&mut e, &format!("{hd}:item:save"));
    assert_eq!(
        fx.first(),
        Some(&OverlayEffect::Intent(OverlayIntent::DropdownItem {
            win: W,
            id: Ov::Menu,
            item: WidgetId::from("save")
        }))
    );
    assert_eq!(closes(&fx), vec![(Ov::Menu, CloseCause::Item)]);

    let rev = e.revision();
    let fx = click(&mut e, &format!("{hm}:tab:3"));
    assert_eq!(
        fx.first(),
        Some(&OverlayEffect::Intent(OverlayIntent::ModalTab {
            win: W,
            id: Ov::Settings,
            index: 3
        }))
    );
    assert_eq!(e.revision(), rev.next());
    assert!(matches!(
        e.view().get(W, Ov::Settings).map(|v| v.body()),
        Some(OverlayBody::Modal(m)) if m.active_tab == 3
    ));
    // Same tab again: intent, but no state change.
    let rev = e.revision();
    click(&mut e, &format!("{hm}:tab:3"));
    assert_eq!(e.revision(), rev);

    // Resize handle: the library consume_event starts the drag; the engine
    // asks for the capture, follows the pointer, releases on up.
    let slot = slot_of(&e, Ov::Settings);
    let fx = e.apply(OverlayOp::Click {
        win: W,
        widget: WidgetId::from(format!("{hm}:resize_se")),
        cursor: Point::new(500.0, 400.0),
    });
    assert!(fx.contains(&OverlayEffect::Capture {
        win: W,
        slot: Some(slot)
    }));
    e.apply(OverlayOp::Captured {
        win: W,
        slot,
        input: PointerInput::Moved {
            pos: Point::new(550.0, 450.0),
            mods: no_mods(),
        },
    });
    assert_eq!(
        e.view().get(W, Ov::Settings).map(|v| v.rect()),
        Some(r(100.0, 100.0, 450.0, 350.0))
    );
    let fx = e.apply(OverlayOp::Captured {
        win: W,
        slot,
        input: PointerInput::Up {
            pos: Point::new(550.0, 450.0),
            button: MouseButton::Left,
            mods: no_mods(),
        },
    });
    assert!(fx.contains(&OverlayEffect::Capture { win: W, slot: None }));

    // Close button: CloseCause::Item.
    let fx = click(&mut e, &format!("{hm}:close"));
    assert_eq!(closes(&fx), vec![(Ov::Settings, CloseCause::Item)]);
    assert!(ids(&e).is_empty());
    assert!(e
        .view()
        .dispatcher(W)
        .is_some_and(|d| d.dispatch(&WidgetId::from(format!("{hm}:close"))).is_none()));
}

#[test]
fn bodies_per_kind_and_custom_picker() {
    let mut e = engine();
    let at = r(10.0, 10.0, 20.0, 20.0);
    open_at(&mut e, Ov::Settings, OverlayKind::Modal, at);
    open_at(&mut e, Ov::Panel, OverlayKind::Popup, at);
    open_at(&mut e, Ov::Menu, OverlayKind::Dropdown, at);
    open_at(&mut e, Ov::Ctx, OverlayKind::ContextMenu, at);
    open_at(&mut e, Ov::Tip, OverlayKind::Tooltip, at);
    open_at(&mut e, Ov::Picker, OverlayKind::ColorPicker, at);
    let body = |id| e.view().get(W, id).map(|v| v.body().clone());
    assert!(matches!(body(Ov::Settings), Some(OverlayBody::Modal(_))));
    assert!(matches!(body(Ov::Panel), Some(OverlayBody::Popup(p)) if p.open));
    assert!(matches!(body(Ov::Menu), Some(OverlayBody::Dropdown(d)) if d.open));
    assert!(matches!(body(Ov::Ctx), Some(OverlayBody::ContextMenu(c)) if c.is_open));
    assert!(matches!(body(Ov::Tip), Some(OverlayBody::Tooltip)));
    assert!(matches!(body(Ov::Picker), Some(OverlayBody::Custom)));
    // Tooltips open no focus scope.
    let fx = open_at(&mut e, Ov::Tip, OverlayKind::Tooltip, at);
    assert!(matches!(
        fx.last(),
        Some(OverlayEffect::Opened {
            scope: None,
            keymap_scope: false,
            ..
        })
    ));
}

// ---------------------------------------------------------------------------
// Keymap scope, window close, snapshot, revision
// ---------------------------------------------------------------------------

#[test]
fn key_verdict_names_top_overlay_only_with_keymap_scope() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 10.0, 10.0),
    );
    let k = key(KeyCode::K, KeyState::Down);
    assert_eq!(
        kroute(&intercept(&mut e, k.clone())),
        KeyRoute::Pass {
            top_overlay: Some(Ov::Menu),
            modal_open: false
        }
    );
    let no_scope = OverlayPolicy {
        keymap_scope: false,
        ..OverlayPolicy::for_kind(OverlayKind::Popup)
    };
    open_with(
        &mut e,
        Ov::Panel,
        OverlayKind::Popup,
        None,
        fixed(5.0, 5.0),
        Some(no_scope),
    );
    assert_eq!(
        kroute(&intercept(&mut e, k.clone())),
        KeyRoute::Pass {
            top_overlay: None,
            modal_open: false
        },
        "the menu below is not on top"
    );
    // A window with no overlays passes with the empty context.
    assert_eq!(
        kroute(&e.apply(OverlayOp::Intercept { win: W2, event: k })),
        KeyRoute::Pass {
            top_overlay: None,
            modal_open: false
        }
    );
}

#[test]
fn window_close_closes_stack_top_down() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 10.0, 10.0),
    );
    modal(&mut e, Ov::Settings, r(0.0, 0.0, 10.0, 10.0));
    let fx = e.apply(OverlayOp::CloseWindow(W));
    assert_eq!(
        closes(&fx),
        vec![
            (Ov::Settings, CloseCause::WindowClosed),
            (Ov::Menu, CloseCause::WindowClosed)
        ],
        "newest first"
    );
    assert!(e.view().is_empty(W));
    assert_eq!(e.view().viewport(W), None);
    let rev = e.revision();
    assert!(e.apply(OverlayOp::CloseWindow(W)).is_empty());
    assert_eq!(e.revision(), rev);
}

#[test]
fn snapshot_rows_bottom_to_top_with_scope_depth() {
    let mut e = engine();
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(0.0, 0.0, 10.0, 10.0),
    );
    modal(&mut e, Ov::Settings, r(0.0, 0.0, 20.0, 20.0));
    open_at(&mut e, Ov::Tip, OverlayKind::Tooltip, r(0.0, 0.0, 5.0, 5.0));
    let rows = e.view().snapshot(W);
    let got: Vec<(Ov, bool, usize)> = rows
        .iter()
        .map(|v| (v.id, v.modal, v.scope_depth))
        .collect();
    assert_eq!(
        got,
        vec![
            (Ov::Menu, false, 1),
            (Ov::Settings, true, 2),
            (Ov::Tip, false, 0)
        ]
    );
    assert!(e.view().snapshot(W2).is_empty());
}

#[test]
fn revision_bumps_exactly_on_state_change() {
    let mut e = E::new();
    assert_eq!(e.revision(), Revision::ZERO);
    // Routing with nothing open changes nothing.
    intercept(&mut e, down(1.0, 1.0));
    intercept(&mut e, up(1.0, 1.0));
    intercept(&mut e, esc());
    assert_eq!(e.revision(), Revision::ZERO);
    // Viewport: +1.
    e.apply(OverlayOp::Reclamp {
        win: W,
        viewport: viewport(),
    });
    let r1 = e.revision();
    assert_eq!(r1, Revision::ZERO.next());
    // Open: +1 (a replace is one op: +1).
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(10.0, 10.0, 100.0, 100.0),
    );
    assert_eq!(e.revision(), r1.next());
    open_at(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        r(10.0, 10.0, 100.0, 100.0),
    );
    let r3 = e.revision();
    assert_eq!(r3, r1.next().next());
    // Hits, moves, presses inside, passed keys: no bump.
    intercept(&mut e, moved(50.0, 50.0));
    intercept(&mut e, down(50.0, 50.0));
    intercept(&mut e, up(50.0, 50.0));
    intercept(&mut e, key(KeyCode::A, KeyState::Down));
    intercept(&mut e, down(500.0, 500.0)); // outside press: remembered, not revisioned
    assert_eq!(e.revision(), r3);
    // A move to the same place / a resize to the same size: no bump.
    let same = OverlayCmd::Move {
        win: W,
        id: Ov::Menu,
        to: Point::new(10.0, 10.0),
    };
    e.apply(OverlayOp::Cmd {
        cmd: same,
        now: t(2.0),
    });
    e.apply(OverlayOp::Resize {
        win: W,
        id: Ov::Menu,
        width: 100.0,
        height: 100.0,
    });
    assert_eq!(e.revision(), r3);
    // Real move / resize: +1 each.
    let mv = OverlayCmd::Move {
        win: W,
        id: Ov::Menu,
        to: Point::new(20.0, 10.0),
    };
    e.apply(OverlayOp::Cmd {
        cmd: mv,
        now: t(2.0),
    });
    e.apply(OverlayOp::Resize {
        win: W,
        id: Ov::Menu,
        width: 120.0,
        height: 100.0,
    });
    assert_eq!(e.revision(), r3.next().next());
    // Ops on missing overlays / windows: no bump.
    let r5 = e.revision();
    e.apply(OverlayOp::Cmd {
        cmd: OverlayCmd::Close {
            win: W,
            id: Ov::Ctx,
        },
        now: t(2.0),
    });
    e.apply(OverlayOp::Cmd {
        cmd: OverlayCmd::CloseTop { win: W2 },
        now: t(2.0),
    });
    e.apply(OverlayOp::Fire {
        win: W,
        slot: OverlaySlot(999),
    });
    e.apply(OverlayOp::Resize {
        win: W,
        id: Ov::Ctx,
        width: 1.0,
        height: 1.0,
    });
    assert_eq!(e.revision(), r5);
    // Close (the outside release of the press above): +1.
    intercept(&mut e, up(500.0, 500.0));
    assert_eq!(e.revision(), r5.next());
    assert!(ids(&e).is_empty());
}

#[test]
fn auto_size_starts_empty_until_resize_and_modal_centres() {
    let mut e = engine();
    open_with(
        &mut e,
        Ov::Settings,
        OverlayKind::Modal,
        None,
        fixed(200.0, 100.0),
        None,
    );
    assert_eq!(
        e.view().get(W, Ov::Settings).map(|v| v.rect()),
        Some(r(300.0, 250.0, 200.0, 100.0))
    );
    open_with(
        &mut e,
        Ov::Menu,
        OverlayKind::Dropdown,
        Some(r(40.0, 20.0, 80.0, 24.0)),
        OverlaySize::Auto,
        None,
    );
    assert_eq!(
        e.view().get(W, Ov::Menu).map(|v| v.rect()),
        Some(r(40.0, 44.0, 0.0, 0.0))
    );
    e.apply(OverlayOp::Resize {
        win: W,
        id: Ov::Menu,
        width: 160.0,
        height: 90.0,
    });
    assert_eq!(
        e.view().get(W, Ov::Menu).map(|v| v.rect()),
        Some(r(40.0, 44.0, 160.0, 90.0))
    );
}
