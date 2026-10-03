//! Pure DOM → uzor helpers shared with hand-assembled apps and the framework
//! web host (design §2.4). `WebWindowProvider` stays the lib provider; these
//! three functions are the reuse surface.

use uzor::input::keyboard::events::KeyCode;
use uzor::input::pointer::state::MouseButton;
use web_sys::{HtmlCanvasElement, WheelEvent};

/// `WheelEvent.deltaMode` line units (DOM spec). `web_sys` exposes the raw
/// `u32` but not the named constants.
const DOM_DELTA_LINE: u32 = 1;
/// `WheelEvent.deltaMode` page units.
const DOM_DELTA_PAGE: u32 = 2;

/// Assumed CSS-pixel height of one "line" for `DOM_DELTA_LINE` wheel events.
/// No DOM API exposes the browser's actual line-scroll height; this matches
/// the common ~16px default line-box.
const ASSUMED_LINE_HEIGHT_PX: f64 = 16.0;

/// Normalize a DOM wheel event's raw delta to CSS pixels regardless of
/// `deltaMode`. Pixel mode passes through, line mode multiplies by
/// [`ASSUMED_LINE_HEIGHT_PX`], page mode multiplies by the canvas's own
/// client size.
///
/// The sign is the DOM sign (positive `deltaY` is downward). Callers that
/// match winit's positive-up convention negate the result themselves.
pub fn normalize_wheel_delta_to_css_px(ev: &WheelEvent, canvas: &HtmlCanvasElement) -> (f64, f64) {
    match ev.delta_mode() {
        DOM_DELTA_LINE => (
            ev.delta_x() * ASSUMED_LINE_HEIGHT_PX,
            ev.delta_y() * ASSUMED_LINE_HEIGHT_PX,
        ),
        DOM_DELTA_PAGE => (
            ev.delta_x() * canvas.client_width() as f64,
            ev.delta_y() * canvas.client_height() as f64,
        ),
        _ => (ev.delta_x(), ev.delta_y()),
    }
}

/// DOM `MouseEvent.button` / `PointerEvent.button` → [`MouseButton`].
pub fn map_mouse_button(b: i16) -> MouseButton {
    match b {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

/// DOM `KeyboardEvent.code` (physical) → [`KeyCode`].
pub fn map_keycode(code: &str) -> KeyCode {
    match code {
        "KeyA" => KeyCode::A,
        "KeyB" => KeyCode::B,
        "KeyC" => KeyCode::C,
        "KeyD" => KeyCode::D,
        "KeyE" => KeyCode::E,
        "KeyF" => KeyCode::F,
        "KeyG" => KeyCode::G,
        "KeyH" => KeyCode::H,
        "KeyI" => KeyCode::I,
        "KeyJ" => KeyCode::J,
        "KeyK" => KeyCode::K,
        "KeyL" => KeyCode::L,
        "KeyM" => KeyCode::M,
        "KeyN" => KeyCode::N,
        "KeyO" => KeyCode::O,
        "KeyP" => KeyCode::P,
        "KeyQ" => KeyCode::Q,
        "KeyR" => KeyCode::R,
        "KeyS" => KeyCode::S,
        "KeyT" => KeyCode::T,
        "KeyU" => KeyCode::U,
        "KeyV" => KeyCode::V,
        "KeyW" => KeyCode::W,
        "KeyX" => KeyCode::X,
        "KeyY" => KeyCode::Y,
        "KeyZ" => KeyCode::Z,
        "Digit0" => KeyCode::Num0,
        "Digit1" => KeyCode::Num1,
        "Digit2" => KeyCode::Num2,
        "Digit3" => KeyCode::Num3,
        "Digit4" => KeyCode::Num4,
        "Digit5" => KeyCode::Num5,
        "Digit6" => KeyCode::Num6,
        "Digit7" => KeyCode::Num7,
        "Digit8" => KeyCode::Num8,
        "Digit9" => KeyCode::Num9,
        "Enter" => KeyCode::Enter,
        "Escape" => KeyCode::Escape,
        "Backspace" => KeyCode::Backspace,
        "Tab" => KeyCode::Tab,
        "Space" => KeyCode::Space,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F9" => KeyCode::F9,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        "F12" => KeyCode::F12,
        "Delete" => KeyCode::Delete,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        _ => KeyCode::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_unknown() {
        assert_eq!(map_keycode("KeyA"), KeyCode::A);
        assert_eq!(map_keycode("Digit0"), KeyCode::Num0);
        assert_eq!(map_keycode("Escape"), KeyCode::Escape);
        assert_eq!(map_keycode("F12"), KeyCode::F12);
        assert_eq!(map_keycode("IntlBackslash"), KeyCode::Unknown);
    }

    #[test]
    fn mouse_buttons() {
        assert_eq!(map_mouse_button(0), MouseButton::Left);
        assert_eq!(map_mouse_button(1), MouseButton::Middle);
        assert_eq!(map_mouse_button(2), MouseButton::Right);
        assert_eq!(map_mouse_button(5), MouseButton::Left);
    }
}
