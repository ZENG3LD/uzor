//! DOM field values → [`InputEvent`] (design §2.1, §2.4).
//!
//! Listeners extract numbers and strings from the event; this module decides
//! the envelope payload. No `web_sys` here, so the mapping is tested on the
//! native host as well as wasm.
//!
//! Pointer `offsetX`/`offsetY` and touch positions passed in are already
//! canvas-local CSS pixels, which are logical pixels. Callers must not divide
//! by the device pixel ratio again (winit reports physical pixels; the DOM
//! does not).

use uzor::input::ModifierKeys;
#[cfg(test)]
use uzor::input::MouseButton;
use uzor_window_web::{map_keycode, map_mouse_button};

use crate::types::bus::{
    ImeInput, InputEvent, KeyInput, KeyState, PointerInput, TouchInput, WheelDelta, WheelInput,
};
use crate::types::window::{Point, SizePx};

/// Modifier bits copied off a DOM event.
pub fn mods(shift: bool, ctrl: bool, alt: bool, meta: bool) -> ModifierKeys {
    ModifierKeys {
        shift,
        ctrl,
        alt,
        meta,
    }
}

/// `pointerdown` / `move` / `up` / `cancel` / `enter` / `leave`.
///
/// `pointercancel` is [`PointerInput::Cancelled`] (the OS took the gesture),
/// not a button release.
pub fn pointer_from_dom(
    kind: &str,
    x: f64,
    y: f64,
    button: i16,
    mods: ModifierKeys,
) -> Option<InputEvent> {
    let pos = Point::new(x, y);
    let button = map_mouse_button(button);
    let event = match kind {
        "pointerdown" => PointerInput::Down { pos, button, mods },
        "pointerup" => PointerInput::Up { pos, button, mods },
        "pointermove" => PointerInput::Moved { pos, mods },
        "pointerenter" => PointerInput::Entered,
        "pointerleave" => PointerInput::Left,
        "pointercancel" => PointerInput::Cancelled,
        _ => return None,
    };
    Some(InputEvent::Pointer(event))
}

/// Raw motion while the cursor is pointer-locked. `movementX`/`movementY`
/// are already CSS pixels.
pub fn raw_delta(dx: f64, dy: f64) -> InputEvent {
    InputEvent::Pointer(PointerInput::RawDelta { dx, dy })
}

/// `keydown` / `keyup`. Printable `key` text is attached only on press and
/// repeat, and only when `allow_text` (false during IME composition or with
/// Ctrl/Meta held).
pub fn key_from_dom(
    kind: &str,
    code: &str,
    key: &str,
    repeat: bool,
    mods: ModifierKeys,
    allow_text: bool,
) -> Option<InputEvent> {
    let state = match kind {
        "keyup" => KeyState::Up,
        "keydown" if repeat => KeyState::Repeat,
        "keydown" => KeyState::Down,
        _ => return None,
    };
    let text = if allow_text && state != KeyState::Up && !mods.ctrl && !mods.meta {
        printable_char(key)
    } else {
        None
    };
    Some(InputEvent::Key(KeyInput {
        code: map_keycode(code),
        text,
        state,
        mods,
    }))
}

fn printable_char(key: &str) -> Option<String> {
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) if !ch.is_control() => Some(key.to_string()),
        _ => None,
    }
}

/// Wheel deltas already normalized to CSS pixels (DOM sign: positive down).
///
/// Negated so a positive Y matches winit `MouseScrollDelta` (positive up),
/// which is what the native mapper forwards unchanged.
pub fn wheel_from_css_px(x: f64, y: f64, dx: f64, dy: f64, mods: ModifierKeys) -> InputEvent {
    InputEvent::Wheel(WheelInput {
        pos: Point::new(x, y),
        delta: WheelDelta::Pixels { x: -dx, y: -dy },
        mods,
    })
}

/// `compositionstart` / `compositionupdate` / `compositionend` (H2 §5.2,
/// missing from `WebWindowProvider`).
pub fn composition_from_dom(kind: &str, data: &str) -> Option<InputEvent> {
    let ime = match kind {
        "compositionstart" => ImeInput::Enabled,
        "compositionupdate" => ImeInput::Preedit {
            text: data.to_string(),
            cursor: None,
        },
        "compositionend" => ImeInput::Commit(data.to_string()),
        _ => return None,
    };
    Some(InputEvent::Ime(ime))
}

/// `touchstart` / `touchmove` / `touchend` / `touchcancel`. `x`/`y` are
/// canvas-local CSS pixels.
pub fn touch_from_local(kind: &str, id: u64, x: f64, y: f64) -> Option<InputEvent> {
    let pos = Point::new(x, y);
    let touch = match kind {
        "touchstart" => TouchInput::Start { id, pos },
        "touchmove" => TouchInput::Move { id, pos },
        "touchend" => TouchInput::End { id, pos },
        "touchcancel" => TouchInput::Cancel { id },
        _ => return None,
    };
    Some(InputEvent::Touch(touch))
}

/// Viewport client coordinates → canvas-local CSS pixels.
pub fn canvas_local(client_x: f64, client_y: f64, origin_x: f64, origin_y: f64) -> Point {
    Point::new(client_x - origin_x, client_y - origin_y)
}

/// CSS pixel size × device pixel ratio → physical pixels. A non-finite or
/// non-positive ratio is treated as 1.
pub fn physical_size(css_w: i32, css_h: i32, dpr: f64) -> (SizePx, f64) {
    let dpr = if dpr.is_finite() && dpr > 0.0 {
        dpr
    } else {
        1.0
    };
    let w = (css_w.max(0) as f64 * dpr).round() as u32;
    let h = (css_h.max(0) as f64 * dpr).round() as u32;
    (SizePx::new(w, h), dpr)
}

/// Which mouse button a test can name without going through the DOM.
#[cfg(test)]
fn button_of(event: &InputEvent) -> Option<MouseButton> {
    match event {
        InputEvent::Pointer(
            PointerInput::Down { button, .. } | PointerInput::Up { button, .. },
        ) => Some(*button),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_down_maps_buttons_and_cancel_is_not_up() {
        let mods = mods(false, false, false, false);
        let down = pointer_from_dom("pointerdown", 3.0, 4.0, 2, mods).expect("down");
        assert_eq!(button_of(&down), Some(MouseButton::Right));
        match down {
            InputEvent::Pointer(PointerInput::Down { pos, .. }) => {
                assert_eq!(pos, Point::new(3.0, 4.0));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            pointer_from_dom("pointercancel", 0.0, 0.0, 0, mods),
            Some(InputEvent::Pointer(PointerInput::Cancelled))
        ));
        assert!(pointer_from_dom("click", 0.0, 0.0, 0, mods).is_none());
    }

    #[test]
    fn key_text_skipped_for_shortcuts_and_ime() {
        let plain = mods(false, false, false, false);
        let ctrl = mods(false, true, false, false);
        match key_from_dom("keydown", "KeyA", "a", false, plain, true) {
            Some(InputEvent::Key(k)) => {
                assert_eq!(k.text.as_deref(), Some("a"));
                assert_eq!(k.state, KeyState::Down);
                assert_eq!(k.code, map_keycode("KeyA"));
            }
            other => panic!("unexpected {other:?}"),
        }
        match key_from_dom("keydown", "KeyA", "a", false, ctrl, true) {
            Some(InputEvent::Key(k)) => assert!(k.text.is_none()),
            other => panic!("unexpected {other:?}"),
        }
        match key_from_dom("keydown", "KeyA", "a", false, plain, false) {
            Some(InputEvent::Key(k)) => assert!(k.text.is_none()),
            other => panic!("unexpected {other:?}"),
        }
        match key_from_dom("keydown", "KeyA", "a", true, plain, true) {
            Some(InputEvent::Key(k)) => assert_eq!(k.state, KeyState::Repeat),
            other => panic!("unexpected {other:?}"),
        }
        match key_from_dom("keyup", "Enter", "Enter", false, plain, true) {
            Some(InputEvent::Key(k)) => {
                assert_eq!(k.state, KeyState::Up);
                assert!(k.text.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn wheel_dom_down_becomes_winit_up() {
        match wheel_from_css_px(1.0, 2.0, 4.0, 16.0, mods(true, false, false, false)) {
            InputEvent::Wheel(w) => match w.delta {
                WheelDelta::Pixels { x, y } => {
                    assert_eq!((x, y), (-4.0, -16.0));
                    assert!(w.mods.shift);
                }
                other => panic!("unexpected {other:?}"),
            },
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn composition_events() {
        assert!(matches!(
            composition_from_dom("compositionstart", ""),
            Some(InputEvent::Ime(ImeInput::Enabled))
        ));
        match composition_from_dom("compositionupdate", "ка") {
            Some(InputEvent::Ime(ImeInput::Preedit { text, cursor })) => {
                assert_eq!(text, "ка");
                assert!(cursor.is_none());
            }
            other => panic!("unexpected {other:?}"),
        }
        match composition_from_dom("compositionend", "ка") {
            Some(InputEvent::Ime(ImeInput::Commit(text))) => assert_eq!(text, "ка"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn touch_and_geometry() {
        match touch_from_local("touchstart", 7, 8.0, 9.0) {
            Some(InputEvent::Touch(TouchInput::Start { id, pos })) => {
                assert_eq!(id, 7);
                assert_eq!(pos, Point::new(8.0, 9.0));
            }
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            touch_from_local("touchcancel", 1, 0.0, 0.0),
            Some(InputEvent::Touch(TouchInput::Cancel { id: 1 }))
        ));
        assert_eq!(canvas_local(15.0, 40.0, 5.0, 10.0), Point::new(10.0, 30.0));
        let (size, dpr) = physical_size(10, 20, 2.0);
        assert_eq!(size, SizePx::new(20, 40));
        assert_eq!(dpr, 2.0);
        let (size, dpr) = physical_size(10, 10, f64::NAN);
        assert_eq!((size.width, dpr), (10, 1.0));
    }
}
